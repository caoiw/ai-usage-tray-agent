// Abertura automatica da janela de sessao (5h) do Codex.
//
// A janela de 5h do Codex so' comeca a contar na primeira mensagem depois que a
// anterior acaba. Com isto ligado, o app percebe que a janela de uma conta esta'
// parada e manda um "Oi" pelo Codex CLI para abrir a proxima — o mesmo recurso que
// o Claude ja' tem (`maybe_reopen_claude_session`), com as diferencas abaixo.
//
// - **A conta e' a do app, nao a do CLI.** O disparo roda com `CODEX_HOME` numa
//   pasta so' do app, que recebe uma copia do login da conta
//   (`codex_auth::copiar_para_cli`). Assim o "Oi" cai sempre na conta vigiada, sem
//   depender de em qual conta o CLI do usuario esta' logado, e nada e' gravado no
//   `~/.codex` dele. A pasta e' apagada a cada disparo.
// - **Liga por conta** (`PrefsConta::sessao_auto`), alem do interruptor geral
//   (`Prefs::sessao_auto_codex`). Tudo no `contas.json`, nao no `config.json`: o
//   formato do `config.json` nao pode mudar.
// - **"Parada" e' diferente do Claude.** A API do Codex devolve a janela parada
//   com uso em 0% e o tempo ate' o reset igual a' duracao inteira (ela "anda" junto
//   com o relogio), ou sem `reset_at`. Ver `estado_da_janela`.
// - **Sem janela de sessao, fica dormente.** A OpenAI ja' suspendeu o limite de 5h
//   uma vez, e ha' planos sem ele. Sem a janela na resposta, nada e' enviado e o
//   status explica; quando ela volta, o recurso volta sozinho.
// - **Disparo barato:** modelo leve e contexto minimo (ver `AJUSTES_DO_DISPARO`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate, Utc};
use serde::Serialize;
use serde_json::json;

use crate::contas::{self, Provedor};
use crate::{
    append_log_line, codex_auth, executar_cli, http_client, lock_snapshot, read_config,
    resolve_cli, sessao_auto_slot_devido, Alvo, AppConfig, RuntimePaths, SessaoAutoConfig,
    SharedState, SESSAO_AUTO_MENSAGEM, SESSAO_AUTO_MODO_AGENDADO,
};

/// Nome do CLI nas mensagens.
const CLI: &str = "Codex CLI";

/// Folga para considerar a janela "cheia" (parada): o tempo ate' o reset pode
/// estar alguns segundos abaixo da duracao entre o calculo do servidor e a leitura.
/// Mesmo valor que o CodexBar usa (e confirmou numa janela parada de verdade).
const FOLGA_JANELA_CHEIA_SEGUNDOS: i64 = 90;

/// Depois de um disparo bem-sucedido, nenhum outro por este tempo. A API do Codex
/// demora a refletir a janela nova, e logo depois do "Oi" ela ainda pode parecer
/// parada (uso arredondado para 0% e o reset quase na duracao inteira).
const ESPERA_APOS_SUCESSO: Duration = Duration::from_secs(15 * 60);

/// Depois de uma falha (CLI ausente, rede, login), nenhuma nova tentativa por este
/// tempo. A falha nao gasta cota, mas tentar de novo a cada ciclo so' enche o log.
const ESPERA_APOS_FALHA: Duration = Duration::from_secs(5 * 60);

/// Depois de uma recusa por limite de uso, espera mais: o limite nao some em
/// minutos.
const ESPERA_APOS_LIMITE: Duration = Duration::from_secs(30 * 60);

/// Modelo do disparo: o mais leve do catalogo do Codex que ainda conta na janela
/// de 5h. Os modelos "mini" sao cobrados de outra cota e nao abrem a janela.
const MODELO_LEVE: &str = "gpt-6-luna";

/// Instrucao de sistema do disparo, no lugar do prompt inteiro do agente: o "Oi"
/// so' precisa existir para abrir a janela.
const INSTRUCOES_DO_DISPARO: &str = "Responda apenas: ok\n";

/// Ajustes que enxugam o contexto do disparo. Medido com o `--json` do CLI: o
/// disparo foi de ~15 mil tokens de entrada para ~2,7 mil, e a saida de 13 para 5.
/// O que sai: provedor que nao seja o da OpenAI, login que nao seja o ChatGPT,
/// raciocinio (esforco no minimo que existe), a lista de skills, as instrucoes de
/// permissoes, ambiente, colaboracao e multiagente, a busca na web e as
/// ferramentas e plugins.
///
/// Tudo via `-c`, e nao `--disable`: um `--disable` com nome que o CLI nao
/// conhece derruba o comando ("Unknown feature flag"), enquanto uma chave
/// desconhecida no `-c` so' gera aviso — assim uma versao nova do CLI que tire
/// alguma dessas opcoes nao quebra o disparo. Valores **sem aspas** (ver
/// `abrir_sessao`).
const AJUSTES_DO_DISPARO: &[&str] = &[
    "model_provider=openai",
    "forced_login_method=chatgpt",
    "model_reasoning_effort=low",
    "skills.include_instructions=false",
    "skills.bundled.enabled=false",
    "include_permissions_instructions=false",
    "include_environment_context=false",
    "include_collaboration_mode_instructions=false",
    "agents.enabled=false",
    "web_search=disabled",
    "features.plugins=false",
    "features.apps=false",
    "features.tool_suggest=false",
    "features.shell_tool=false",
    "features.unified_exec=false",
    "features.view_image=false",
    "features.image_generation=false",
    "features.browser_use=false",
    "features.computer_use=false",
    "features.multi_agent=false",
    "features.goals=false",
    "features.sleep_tool=false",
    "features.hooks=false",
    "features.mentions_v2=false",
];

/// Uma janela de limite do Codex, como a API devolve.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JanelaCodex {
    /// Uso da janela, em %.
    pub usado: Option<f64>,
    /// Duracao da janela (`limit_window_seconds`).
    pub limite_segundos: Option<i64>,
    /// Segundos ate' o reset, calculados pelo servidor na hora da leitura.
    pub reset_em_segundos: Option<i64>,
    /// Instante do reset (unix, segundos).
    pub reset_em: Option<i64>,
}

/// O que a coleta do Codex leu de uma conta e a sessao automatica precisa.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeituraCodex {
    /// A janela de sessao (a de menor duracao, abaixo de 1 dia). `None` quando a
    /// resposta so' traz a semanal.
    pub sessao: Option<JanelaCodex>,
    /// Uso da janela semanal, em %.
    pub semanal_usado: Option<f64>,
    /// A API diz que algum limite bloqueia o uso agora (`allowed: false` ou
    /// `limit_reached: true`).
    pub bloqueado: bool,
}

/// Situacao da janela de sessao de uma conta, para decidir o disparo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EstadoJanela {
    /// Ninguem abriu a janela ainda: e' a hora do "Oi".
    Parada,
    /// Ha' uma janela correndo.
    Aberta,
    /// A resposta nao traz janela de sessao (limite de 5h suspenso ou plano sem
    /// ele). O recurso fica dormente.
    SemJanelaDeSessao,
    /// A cota semanal (ou outro limite) esta' esgotada: um "Oi" nao teria efeito.
    CotaEsgotada,
    /// A resposta nao basta para decidir com seguranca (ex.: sem a duracao).
    Indeterminado,
}

/// Classifica a janela de sessao da `leitura` no instante `agora` (unix, segundos).
///
/// "Parada" e':
/// - o reset ja' passou (a janela anterior acabou); ou
/// - uso em 0% e o tempo ate' o reset ainda na duracao inteira (menos a folga) —
///   a janela "anda" com o relogio enquanto ninguem a abre; ou
/// - uso em 0% sem nenhuma informacao de reset.
///
/// Logo depois de um disparo a janela pode continuar parecendo parada (uso
/// arredondado para 0%); quem protege disso e' a espera apos o sucesso.
pub fn estado_da_janela(leitura: &LeituraCodex, agora: i64) -> EstadoJanela {
    let Some(sessao) = &leitura.sessao else {
        return EstadoJanela::SemJanelaDeSessao;
    };
    // Sem a duracao, a janela foi classificada como de sessao so' pela posicao na
    // resposta: nao da' para aplicar a regra de "cheia" com seguranca.
    let Some(limite) = sessao.limite_segundos.filter(|segundos| *segundos > 0) else {
        return EstadoJanela::Indeterminado;
    };
    if leitura.semanal_usado.is_some_and(|usado| usado >= 100.0) {
        return EstadoJanela::CotaEsgotada;
    }
    // Tempo ate' o reset: o do servidor quando houver (o relogio do PC pode estar
    // fora, e um "Oi" antes da hora cairia na janela velha); senao, pelo instante
    // do reset.
    let restante = sessao
        .reset_em_segundos
        .or_else(|| sessao.reset_em.map(|reset| reset - agora));
    // A janela acabou. Vem antes do bloqueio: logo depois do reset a resposta ainda
    // pode trazer o `limit_reached` da janela que venceu.
    if restante.is_some_and(|restante| restante <= 0) {
        return EstadoJanela::Parada;
    }
    // Bloqueio com a janela de sessao ainda livre: e' outro limite (o semanal, ou
    // um que a resposta nao detalha). Com a sessao no teto, e' so' ela que esta'
    // cheia, e a janela esta' aberta.
    if leitura.bloqueado && sessao.usado.is_none_or(|usado| usado < 100.0) {
        return EstadoJanela::CotaEsgotada;
    }
    let Some(usado) = sessao.usado else {
        return EstadoJanela::Indeterminado;
    };
    if usado > 0.0 {
        return EstadoJanela::Aberta;
    }
    match restante {
        None => EstadoJanela::Parada,
        Some(restante) if restante >= limite - FOLGA_JANELA_CHEIA_SEGUNDOS => EstadoJanela::Parada,
        Some(_) => EstadoJanela::Aberta,
    }
}

/// Estado da sessao automatica de todas as contas. So' em memoria.
#[derive(Default)]
pub struct Estado {
    contas: Mutex<BTreeMap<String, EstadoConta>>,
    /// Um disparo por vez, mesmo com varias contas: cada um e' um processo do CLI.
    fila: Mutex<()>,
}

/// Estado em memoria de uma conta.
#[derive(Debug, Default)]
struct EstadoConta {
    /// Prazo da espera apos o ultimo envio.
    espera_ate: Option<Instant>,
    /// Ultimo horario agendado que ja' disparou (ver `sessao_auto_slot_devido`).
    ultimo_horario: Option<(NaiveDate, u32)>,
    em_execucao: bool,
    ultima_tentativa_em: Option<String>,
    /// Resultado da ultima tentativa (`Err` com o motivo da falha).
    ultimo_resultado: Option<Result<(), String>>,
}

impl EstadoConta {
    fn esperando(&self) -> bool {
        self.espera_ate.is_some_and(|prazo| Instant::now() < prazo)
    }
}

/// Status de uma conta para a aba Codex.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusConta {
    pub ultima_tentativa_em: Option<String>,
    pub ultimo_ok: Option<bool>,
    pub ultimo_erro: Option<String>,
    pub em_execucao: bool,
    /// Por que nada esta' sendo enviado agora, quando ha' um motivo.
    pub aviso: Option<String>,
}

fn travar(estado: &Estado) -> MutexGuard<'_, BTreeMap<String, EstadoConta>> {
    estado
        .contas
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Leitura da coleta mais recente da conta, se ela deu certo.
fn leitura_da_conta(shared: &SharedState, chave: &str) -> Option<LeituraCodex> {
    lock_snapshot(shared)
        .metrics
        .get(chave)
        .filter(|metrica| metrica.status == "ok")
        .and_then(|metrica| metrica.leitura_codex.clone())
}

/// Status da conta para a UI: o resultado da ultima tentativa e o motivo de estar
/// parada, se houver.
pub fn status_da_conta(shared: &SharedState, chave: &str) -> StatusConta {
    let aviso = leitura_da_conta(shared, chave).and_then(|leitura| {
        match estado_da_janela(&leitura, Utc::now().timestamp()) {
            EstadoJanela::SemJanelaDeSessao => Some(
                "A janela de 5h não está ativa nesta conta agora. A abertura automática volta sozinha quando ela reaparecer."
                    .to_string(),
            ),
            EstadoJanela::CotaEsgotada => {
                Some("Cota esgotada: nada é enviado até ela renovar.".to_string())
            }
            EstadoJanela::Indeterminado => Some(
                "Não foi possível ler a janela de 5h desta conta; nada é enviado.".to_string(),
            ),
            EstadoJanela::Parada | EstadoJanela::Aberta => None,
        }
    });
    let estado = travar(&shared.sessao_auto_codex);
    let conta = estado.get(chave);
    let resultado = conta.and_then(|conta| conta.ultimo_resultado.as_ref());
    StatusConta {
        ultima_tentativa_em: conta.and_then(|conta| conta.ultima_tentativa_em.clone()),
        ultimo_ok: resultado.map(Result::is_ok),
        ultimo_erro: resultado.and_then(|resultado| resultado.clone().err()),
        em_execucao: conta.is_some_and(|conta| conta.em_execucao),
        aviso,
    }
}

/// Decide, para cada conta do Codex com o recurso ligado, se e' hora de abrir a
/// janela, e dispara fora do ciclo de coleta (o CLI leva segundos).
///
/// So' age sobre uma coleta bem-sucedida: falha de coleta nunca vira disparo.
/// Recebe as preferencias e as contas que o ciclo de coleta ja' leu do disco.
pub fn talvez_abrir(
    paths: &RuntimePaths,
    shared: &Arc<SharedState>,
    config: &AppConfig,
    prefs: &contas::Prefs,
    alvos: &[Alvo],
) {
    if !config.providers.codex.habilitado {
        return;
    }
    let ajustes = prefs.sessao_auto_codex();
    if !ajustes.habilitado {
        return;
    }
    let agora = Utc::now().timestamp();
    for conta in alvos.iter().filter(|alvo| alvo.provedor == Provedor::Codex) {
        // O provedor sem conta entra com a chave dele ("codex"), que nunca esta'
        // ligada nas preferencias.
        if !prefs.sessao_auto(&conta.chave) {
            continue;
        }
        let Some(leitura) = leitura_da_conta(shared, &conta.chave) else {
            continue;
        };
        if estado_da_janela(&leitura, agora) == EstadoJanela::Parada
            && pode_disparar(
                &shared.sessao_auto_codex,
                &ajustes,
                config.intervalo_segundos,
                &conta.chave,
            )
        {
            disparar(paths, shared, conta.chave.clone(), false);
        }
    }
}

/// Com a janela parada, decide se ha' disparo agora (nada em andamento, fora da
/// espera e, no modo agendado, no horario). Marca a tentativa com a trava tomada
/// quando a resposta e' sim, para dois ciclos nao dispararem o mesmo "Oi".
fn pode_disparar(
    contas_do_recurso: &Estado,
    ajustes: &SessaoAutoConfig,
    intervalo_segundos: u64,
    chave: &str,
) -> bool {
    let mut contas_estado = travar(contas_do_recurso);
    let conta = contas_estado.entry(chave.to_string()).or_default();
    if conta.em_execucao || conta.esperando() {
        return false;
    }

    // Modo agendado: so' no horario (horario perdido nao e' recuperado), uma vez
    // por horario.
    if ajustes.modo == SESSAO_AUTO_MODO_AGENDADO {
        let agora = Local::now();
        let Some(minuto) = sessao_auto_slot_devido(ajustes, intervalo_segundos, agora) else {
            return false;
        };
        let horario = Some((agora.date_naive(), minuto));
        if conta.ultimo_horario == horario {
            return false;
        }
        conta.ultimo_horario = horario;
    }

    marcar_inicio(conta);
    true
}

fn marcar_inicio(conta: &mut EstadoConta) {
    conta.em_execucao = true;
}

/// "Enviar teste": manda o "Oi" agora, sem olhar a janela nem a espera (o usuario
/// pediu). A guarda de cobranca continua valendo.
pub fn testar(
    paths: &RuntimePaths,
    shared: &Arc<SharedState>,
    config: &AppConfig,
    chave: &str,
) -> Result<(), String> {
    // O provedor desligado nas Configuracoes nao envia nada, nem em teste.
    if !config.providers.codex.habilitado {
        return Err("O Codex está desativado nas Configurações.".to_string());
    }
    if arquivo_da_conta(paths, chave).is_none() {
        return Err("Conta não encontrada.".to_string());
    }
    {
        let mut estado = travar(&shared.sessao_auto_codex);
        let atual = estado.entry(chave.to_string()).or_default();
        if atual.em_execucao {
            return Err("Já há um envio em andamento nesta conta.".to_string());
        }
        marcar_inicio(atual);
    }
    disparar(paths, shared, chave.to_string(), true);
    Ok(())
}

/// Roda o disparo numa thread e registra o resultado. A tentativa ja' foi marcada
/// (`marcar_inicio`) por quem chama.
fn disparar(paths: &RuntimePaths, shared: &Arc<SharedState>, chave: String, manual: bool) {
    let paths = paths.clone();
    let shared = shared.clone();
    thread::spawn(move || executar_disparo(&paths, &shared.sessao_auto_codex, &chave, manual));
}

/// Valida depois da fila, envia e registra. Separado da thread para o teste poder
/// aguardar o termino sem polling nem montar todo o estado do aplicativo.
fn executar_disparo(paths: &RuntimePaths, recurso: &Estado, chave: &str, manual: bool) {
    let fila = recurso.fila.lock().unwrap_or_else(|p| p.into_inner());
    let config = read_config(paths);
    let prefs = contas::ler_prefs(&paths.config_dir);
    let ajustes = prefs.sessao_auto_codex();
    // O teste manual ignora os opt-ins, mas tambem respeita o provedor desligado.
    if !config.providers.codex.habilitado
        || (!manual && (!ajustes.habilitado || !prefs.sessao_auto(chave)))
    {
        let mut estado = travar(recurso);
        let conta = estado.entry(chave.to_string()).or_default();
        conta.em_execucao = false;
        conta.espera_ate = None;
        conta.ultimo_horario = None;
        return;
    }
    // So' altera o historico quando o envio realmente comeca; cancelar a fila
    // preserva o resultado e a data da tentativa anterior.
    travar(recurso)
        .entry(chave.to_string())
        .or_default()
        .ultima_tentativa_em = Some(Utc::now().to_rfc3339());
    // Resolve a conta pela chave agora: a principal pode ter mudado durante a fila.
    // Um panic vira falha para nao deixar a conta presa "em execucao".
    let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        abrir_sessao(paths, chave, &ajustes)
    }))
    .unwrap_or_else(|_| Err("falha inesperada no disparo.".to_string()));
    drop(fila);

    let mut estado = travar(recurso);
    let conta = estado.entry(chave.to_string()).or_default();
    conta.em_execucao = false;
    let espera = match &resultado {
        Ok(()) => ESPERA_APOS_SUCESSO,
        Err(erro) if falha_por_limite(erro) => ESPERA_APOS_LIMITE,
        Err(_) => ESPERA_APOS_FALHA,
    };
    conta.espera_ate = Some(Instant::now() + espera);
    if resultado.is_err() {
        // A falha nao consome o horario: pode tentar de novo dentro da folga dele.
        conta.ultimo_horario = None;
    }
    let novo = conta.ultimo_resultado.as_ref() != Some(&resultado);
    conta.ultimo_resultado = Some(resultado.clone());
    drop(estado);

    let (nivel, mensagem, detalhe) = match resultado {
        Ok(()) => (
            "info",
            "Sessao do Codex aberta automaticamente.",
            json!({ "ferramenta": "codex", "conta": chave, "mensagem": SESSAO_AUTO_MENSAGEM }),
        ),
        Err(erro) if novo => (
            "error",
            "Falha ao abrir a sessao do Codex.",
            json!({ "ferramenta": "codex", "conta": chave, "error": erro }),
        ),
        Err(_) => return, // Nao repete a mesma falha no log a cada espera.
    };
    let _ = append_log_line(paths, nivel, mensagem, Some(detalhe));
}

/// A falha foi o limite de uso (a mensagem do CLI e' "You've hit your usage
/// limit…"). O exit code e' o mesmo de qualquer erro, entao so' o texto diz.
fn falha_por_limite(erro: &str) -> bool {
    let erro = erro.to_lowercase();
    erro.contains("usage limit") || erro.contains("rate limit")
}

/// Explica a falha quando o motivo e' conhecido; senao, devolve como veio.
fn explicar_falha(erro: String) -> String {
    let minusculo = erro.to_lowercase();
    if minusculo.contains("unexpected argument") || minusculo.contains("unrecognized") {
        return format!(
            "o {CLI} instalado é antigo demais para este recurso. Atualize-o (npm i -g @openai/codex). Detalhe: {erro}"
        );
    }
    if falha_por_limite(&erro) {
        return format!("limite de uso atingido. Detalhe: {erro}");
    }
    erro
}

/// Arquivo de login da conta `chave` agora. Procurado na hora, e nao guardado: a
/// conta principal muda trocando os arquivos de lugar.
fn arquivo_da_conta(paths: &RuntimePaths, chave: &str) -> Option<PathBuf> {
    contas::listar(&paths.config_dir, Provedor::Codex)
        .into_iter()
        .find(|conta| conta.chave == chave)
        .map(|conta| conta.arquivo)
}

/// Pasta do disparo: vira o `CODEX_HOME` do CLI (com a copia do login) e e'
/// apagada a cada disparo.
fn pasta_do_disparo(paths: &RuntimePaths) -> PathBuf {
    paths.config_dir.join("sessao-auto-codex")
}

/// Manda o "Oi" pelo Codex CLI na conta `chave`.
///
/// - renova o token antes (`ensure_fresh`): a copia leva so' o `access_token`;
/// - copia o login para `<pasta>/auth.json` e aponta o `CODEX_HOME` para la' (a
///   copia so' sai se for login ChatGPT, nunca chave de API);
/// - roda numa subpasta vazia, sem config do usuario (o `CODEX_HOME` novo nao tem
///   `config.toml`), no modelo mais leve, com o contexto enxuto e sandbox so' de
///   leitura (ver `MODELO_LEVE` e `AJUSTES_DO_DISPARO`);
/// - os valores do `-c` vao **sem aspas**: no Windows o CLI costuma ser o shim
///   `codex.cmd`, que corrompe aspas (ver a armadilha do `claude.cmd` no
///   ONBOARDING). Sem aspas, o CLI le' o valor como texto puro;
/// - tira do ambiente as variaveis que desviariam para a API paga;
/// - no fim, apaga a pasta (e a copia do login com ela).
fn abrir_sessao(
    paths: &RuntimePaths,
    chave: &str,
    config: &SessaoAutoConfig,
) -> Result<(), String> {
    let exe = resolve_codex_cli(config)?;
    let arquivo = arquivo_da_conta(paths, chave)
        .ok_or_else(|| "a conta não está mais conectada no app.".to_string())?;
    codex_auth::ensure_fresh(&http_client(), &arquivo)?;

    let pasta = pasta_do_disparo(paths);
    // Sobra de um disparo interrompido (app fechado no meio): comeca do zero.
    let _ = fs::remove_dir_all(&pasta);
    let vazia = pasta.join("vazia");
    fs::create_dir_all(&vazia).map_err(|error| {
        format!(
            "nao foi possivel criar a pasta do disparo em {}: {error}",
            pasta.display()
        )
    })?;

    let resultado =
        codex_auth::copiar_para_cli(&arquivo, &pasta.join("auth.json")).and_then(|identidade| {
            // O arquivo pode ter trocado de conta entre achar o caminho e copiar (a
            // principal muda trocando os arquivos de lugar). O "Oi" so' sai se o
            // login copiado for mesmo o da conta pedida: senao gastaria a cota de
            // outra.
            if contas::chave_da_identidade(Provedor::Codex, &identidade) != chave {
                return Err(
                    "a conta principal mudou durante o envio; nada foi enviado.".to_string()
                );
            }
            let instrucoes = pasta.join("instrucoes.md");
            fs::write(&instrucoes, INSTRUCOES_DO_DISPARO).map_err(|error| {
                format!("nao foi possivel gravar as instrucoes do disparo: {error}")
            })?;
            executar_cli(comando(&exe, &pasta, &vazia, &instrucoes), CLI)
        });

    // Sempre: a copia do login nao pode ficar no disco. Se nao der para apagar
    // agora (arquivo ainda em uso), fica registrado, e o proximo disparo tenta de
    // novo antes de comecar.
    if let Err(error) = fs::remove_dir_all(&pasta) {
        if error.kind() != std::io::ErrorKind::NotFound {
            let _ = append_log_line(
                paths,
                "error",
                "Nao foi possivel apagar a pasta do disparo do Codex (com a copia do login).",
                Some(
                    json!({ "ferramenta": "codex", "pasta": pasta.display().to_string(), "error": error.to_string() }),
                ),
            );
        }
    }
    resultado.map_err(explicar_falha)
}

/// Monta o `codex exec` do disparo (ver `abrir_sessao`).
fn comando(exe: &Path, pasta: &Path, vazia: &Path, instrucoes: &Path) -> Command {
    let mut command = Command::new(exe);
    command
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("--ephemeral")
        .args(["--sandbox", "read-only"])
        .arg("-C")
        .arg(vazia)
        .args(["-m", MODELO_LEVE]);
    for ajuste in AJUSTES_DO_DISPARO {
        command.args(["-c", ajuste]);
    }
    // A instrucao de sistema troca o prompt do agente inteiro. Vai como caminho de
    // arquivo, que atravessa o shim `.cmd` intacto (com espacos e tudo).
    let mut arquivo_de_instrucoes = std::ffi::OsString::from("model_instructions_file=");
    arquivo_de_instrucoes.push(instrucoes);
    command
        .arg("-c")
        .arg(arquivo_de_instrucoes)
        .arg(SESSAO_AUTO_MENSAGEM)
        .current_dir(vazia)
        .env("CODEX_HOME", pasta)
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_BASE_URL");
    command
}

/// Descobre o executavel do Codex CLI: o `caminho_cli` do config, senao os locais
/// conhecidos de instalacao (antes do PATH: o app costuma subir pelo autostart,
/// com um PATH incompleto), senao o PATH.
fn resolve_codex_cli(config: &SessaoAutoConfig) -> Result<PathBuf, String> {
    resolve_cli(
        CLI,
        "codex",
        &config.caminho_cli,
        candidatos(),
        "Instale-o (npm i -g @openai/codex) e, se preciso, informe o caminho no contas.json (sessaoAutoCodex.caminhoCli).",
    )
}

/// Locais padrao de instalacao do Codex CLI, por sistema.
fn candidatos() -> Vec<PathBuf> {
    let mut candidatos = Vec::new();
    let home = dirs::home_dir();

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let appdata = PathBuf::from(appdata);
            // npm global padrao.
            candidatos.push(appdata.join("npm").join("codex.cmd"));
            // npm global de quem usa o fnm: a versao padrao do Node fica num alias
            // estavel (o PATH do shell aponta para uma pasta temporaria por sessao).
            candidatos.push(
                appdata
                    .join("fnm")
                    .join("aliases")
                    .join("default")
                    .join("codex.cmd"),
            );
        }
        if let Some(home) = home.as_ref() {
            candidatos.push(home.join(".local").join("bin").join("codex.exe"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = home.as_ref() {
            candidatos.push(home.join(".local").join("bin").join("codex"));
            candidatos.push(home.join(".npm-global").join("bin").join("codex"));
        }
        candidatos.push(PathBuf::from("/usr/local/bin/codex"));
        candidatos.push(PathBuf::from("/usr/bin/codex"));
    }

    candidatos
}

#[cfg(test)]
mod tests {
    use super::*;

    const CINCO_HORAS: i64 = 5 * 60 * 60;
    const AGORA: i64 = 1_800_000_000;

    fn leitura(usado: Option<f64>, reset_em_segundos: Option<i64>) -> LeituraCodex {
        LeituraCodex {
            sessao: Some(JanelaCodex {
                usado,
                limite_segundos: Some(CINCO_HORAS),
                reset_em_segundos,
                reset_em: reset_em_segundos.map(|segundos| AGORA + segundos),
            }),
            semanal_usado: Some(40.0),
            bloqueado: false,
        }
    }

    #[test]
    fn janela_parada_e_a_que_anda_com_o_relogio() {
        // Uso zero e o reset na duracao inteira: ninguem abriu a janela.
        assert_eq!(
            estado_da_janela(&leitura(Some(0.0), Some(CINCO_HORAS)), AGORA),
            EstadoJanela::Parada
        );
        // Dentro da folga ainda e' parada.
        assert_eq!(
            estado_da_janela(&leitura(Some(0.0), Some(CINCO_HORAS - 60)), AGORA),
            EstadoJanela::Parada
        );
        // Uso zero sem nenhuma informacao de reset.
        let mut sem_reset = leitura(Some(0.0), None);
        assert_eq!(estado_da_janela(&sem_reset, AGORA), EstadoJanela::Parada);
        // O reset ja' passou: a janela anterior acabou.
        sem_reset.sessao.as_mut().unwrap().reset_em = Some(AGORA - 1);
        sem_reset.sessao.as_mut().unwrap().usado = Some(80.0);
        assert_eq!(estado_da_janela(&sem_reset, AGORA), EstadoJanela::Parada);
    }

    #[test]
    fn janela_aberta_tem_uso_ou_ja_andou() {
        assert_eq!(
            estado_da_janela(&leitura(Some(12.0), Some(3_600)), AGORA),
            EstadoJanela::Aberta
        );
        // Uso arredondado para 0%, mas o reset ja' andou: alguem abriu a janela.
        assert_eq!(
            estado_da_janela(&leitura(Some(0.0), Some(CINCO_HORAS - 600)), AGORA),
            EstadoJanela::Aberta
        );
        // Sessao no teto com o bloqueio: e' ela que esta' cheia, nao outra cota.
        let mut cheia = leitura(Some(100.0), Some(7_000));
        cheia.bloqueado = true;
        assert_eq!(estado_da_janela(&cheia, AGORA), EstadoJanela::Aberta);
    }

    #[test]
    fn sem_janela_de_sessao_fica_dormente() {
        // So' a semanal na resposta (5h suspenso ou plano sem ele).
        let mut so_semanal = leitura(Some(0.0), Some(CINCO_HORAS));
        so_semanal.sessao = None;
        assert_eq!(
            estado_da_janela(&so_semanal, AGORA),
            EstadoJanela::SemJanelaDeSessao
        );
    }

    #[test]
    fn duracao_diferente_de_5h_usa_a_duracao_real() {
        let mut tres_horas = leitura(Some(0.0), Some(3 * 3_600));
        tres_horas.sessao.as_mut().unwrap().limite_segundos = Some(3 * 3_600);
        assert_eq!(estado_da_janela(&tres_horas, AGORA), EstadoJanela::Parada);
        tres_horas.sessao.as_mut().unwrap().reset_em_segundos = Some(3_600);
        assert_eq!(estado_da_janela(&tres_horas, AGORA), EstadoJanela::Aberta);
    }

    #[test]
    fn cota_esgotada_ou_bloqueio_nao_disparam() {
        let mut semanal_cheia = leitura(Some(0.0), Some(CINCO_HORAS));
        semanal_cheia.semanal_usado = Some(100.0);
        assert_eq!(
            estado_da_janela(&semanal_cheia, AGORA),
            EstadoJanela::CotaEsgotada
        );
        let mut bloqueada = leitura(Some(0.0), Some(CINCO_HORAS));
        bloqueada.bloqueado = true;
        assert_eq!(
            estado_da_janela(&bloqueada, AGORA),
            EstadoJanela::CotaEsgotada
        );
    }

    #[test]
    fn sem_a_duracao_nao_decide() {
        let mut sem_duracao = leitura(Some(0.0), Some(CINCO_HORAS));
        sem_duracao.sessao.as_mut().unwrap().limite_segundos = None;
        assert_eq!(
            estado_da_janela(&sem_duracao, AGORA),
            EstadoJanela::Indeterminado
        );
        let mut sem_uso = leitura(None, Some(CINCO_HORAS));
        sem_uso.sessao.as_mut().unwrap().reset_em = None;
        assert_eq!(
            estado_da_janela(&sem_uso, AGORA),
            EstadoJanela::Indeterminado
        );
    }

    /// Com o relogio do PC adiantado, o `reset_at` ja' "passou" aqui mas o servidor
    /// ainda conta tempo: vale o servidor, e a janela continua aberta.
    #[test]
    fn relogio_adiantado_nao_encerra_a_janela_antes_do_servidor() {
        let mut adiantado = leitura(Some(80.0), Some(120));
        adiantado.sessao.as_mut().unwrap().reset_em = Some(AGORA - 60);
        assert_eq!(estado_da_janela(&adiantado, AGORA), EstadoJanela::Aberta);
        // O servidor diz que acabou: parada, seja qual for o relogio local.
        adiantado.sessao.as_mut().unwrap().reset_em_segundos = Some(0);
        assert_eq!(estado_da_janela(&adiantado, AGORA), EstadoJanela::Parada);
        // Logo depois do reset a resposta ainda pode trazer o bloqueio da janela
        // que venceu: a janela acabou, entao e' parada, e nao cota esgotada.
        adiantado.bloqueado = true;
        assert_eq!(estado_da_janela(&adiantado, AGORA), EstadoJanela::Parada);
    }

    /// Um disparo por vez por conta, e nada durante a espera depois dele.
    #[test]
    fn nao_dispara_com_envio_em_andamento_ou_na_espera() {
        let estado = Estado::default();
        let ajustes = SessaoAutoConfig::default();
        assert!(pode_disparar(&estado, &ajustes, 10, "codex:a"));
        // A tentativa ficou marcada: em andamento.
        assert!(!pode_disparar(&estado, &ajustes, 10, "codex:a"));
        // Terminou, mas a espera ainda corre.
        {
            let mut contas = travar(&estado);
            let conta = contas.get_mut("codex:a").unwrap();
            conta.em_execucao = false;
            conta.espera_ate = Some(Instant::now() + ESPERA_APOS_FALHA);
        }
        assert!(!pode_disparar(&estado, &ajustes, 10, "codex:a"));
        // Outra conta nao e' afetada.
        assert!(pode_disparar(&estado, &ajustes, 10, "codex:b"));
    }

    /// Pasta de teste com uma conta do Codex valida (token longe de expirar: o
    /// `ensure_fresh` nao vai a' rede) e um `codex.cmd` falso com o corpo dado.
    #[cfg(target_os = "windows")]
    fn ambiente_com_cli_falso(nome: &str, corpo: &str) -> (RuntimePaths, String, SessaoAutoConfig) {
        let base = std::env::temp_dir().join(format!(
            "ai-usage-sessao-codex-{nome}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("criar a pasta do teste");
        let validade = Utc::now().timestamp_millis() + 24 * 60 * 60 * 1000;
        fs::write(
            base.join("codex-auth.json"),
            json!({
                "tokens": {
                    "id_token": "id",
                    "access_token": "acesso",
                    "refresh_token": "r1",
                    "account_id": "conta-1"
                },
                "expires_at": validade
            })
            .to_string(),
        )
        .expect("gravar a conta de teste");
        let falso = base.join("codex-falso.cmd");
        fs::write(&falso, format!("@echo off\r\n{corpo}")).expect("gravar o CLI falso");
        let paths = RuntimePaths {
            config_dir: base.clone(),
            config_file: base.join("config.json"),
            logs_dir: base.join("logs"),
            config_do_cli: Vec::new(),
        };
        let ajustes = SessaoAutoConfig {
            habilitado: true,
            caminho_cli: falso.to_string_lossy().into_owned(),
            ..SessaoAutoConfig::default()
        };
        let chave = contas::listar(&base, Provedor::Codex)
            .into_iter()
            .next()
            .expect("a conta de teste aparece na lista")
            .chave;
        (paths, chave, ajustes)
    }

    /// Mudancas durante a espera cancelam o automatico; o teste manual ignora
    /// os opt-ins, mas tambem respeita o provedor desligado.
    #[cfg(target_os = "windows")]
    #[test]
    fn revalida_os_interruptores_depois_da_fila() {
        for (nome, provedor, geral, conta, manual, envia) in [
            ("fila-provedor", false, true, true, false, false),
            ("fila-geral", true, false, true, false, false),
            ("fila-conta", true, true, false, false, false),
            ("fila-ligada", true, true, true, false, true),
            ("fila-manual", true, false, false, true, true),
            ("fila-manual-provedor", false, true, true, true, false),
        ] {
            let (paths, chave, ajustes) =
                ambiente_com_cli_falso(nome, "echo sim> \"%~dp0enviou.txt\"\r\nexit /b 0\r\n");
            let mut config = AppConfig::default();
            config.providers.codex.habilitado = true;
            crate::write_config(&paths, &config).unwrap();
            let mut prefs = contas::Prefs::default();
            prefs.sessao_auto_codex = Some(ajustes);
            prefs.contas.entry(chave.clone()).or_default().sessao_auto = Some(true);
            // Grava o caminho do CLI como uma edicao manual do contas.json.
            let gravar_prefs = |prefs: &contas::Prefs| {
                fs::write(
                    paths.config_dir.join("contas.json"),
                    serde_json::to_string(prefs).unwrap(),
                )
                .unwrap();
            };
            gravar_prefs(&prefs);
            let recurso = Estado::default();
            let anterior = "2026-01-01T00:00:00Z";
            {
                let mut estado = travar(&recurso);
                let conta = estado.entry(chave.clone()).or_default();
                conta.ultima_tentativa_em = Some(anterior.to_string());
                marcar_inicio(conta);
            }
            thread::scope(|scope| {
                let fila = recurso.fila.lock().unwrap();
                let envio = scope.spawn(|| executar_disparo(&paths, &recurso, &chave, manual));
                // O envio nao pode sair antes destas edicoes.
                config.providers.codex.habilitado = provedor;
                crate::write_config(&paths, &config).unwrap();
                prefs.sessao_auto_codex.as_mut().unwrap().habilitado = geral;
                prefs.contas.get_mut(&chave).unwrap().sessao_auto = Some(conta);
                gravar_prefs(&prefs);
                drop(fila);
                envio.join().unwrap();
            });
            let estado = travar(&recurso);
            assert!(!estado[&chave].em_execucao, "{nome}");
            if envia {
                assert_eq!(estado[&chave].ultimo_resultado, Some(Ok(())), "{nome}");
            }
            assert_eq!(
                paths.config_dir.join("enviou.txt").exists(),
                envia,
                "{nome}"
            );
            if !envia {
                assert!(!estado[&chave].esperando(), "{nome}");
                assert!(estado[&chave].ultimo_resultado.is_none(), "{nome}");
                assert_eq!(
                    estado[&chave].ultima_tentativa_em.as_deref(),
                    Some(anterior)
                );
            }
        }
    }

    /// O disparo roda na conta do app (copia do login no `CODEX_HOME`), com os
    /// argumentos intactos atraves do shim `.cmd`, e nao deixa a copia no disco.
    #[cfg(target_os = "windows")]
    #[test]
    fn disparo_usa_o_login_da_conta_e_apaga_a_copia() {
        let (paths, chave, ajustes) = ambiente_com_cli_falso(
            "ok",
            "echo %* > \"%~dp0args.txt\"\r\n\
             if exist \"%CODEX_HOME%\\auth.json\" echo sim> \"%~dp0auth.txt\"\r\n\
             exit /b 0\r\n",
        );

        abrir_sessao(&paths, &chave, &ajustes).expect("o disparo devia dar certo");

        let args = fs::read_to_string(paths.config_dir.join("args.txt")).expect("o CLI rodou");
        for esperado in [
            "exec",
            "--ephemeral",
            "--skip-git-repo-check",
            "model_reasoning_effort=low",
            "forced_login_method=chatgpt",
            "skills.include_instructions=false",
            "features.plugins=false",
            "-m gpt-6-luna",
            "model_instructions_file=",
            "Oi",
        ] {
            assert!(args.contains(esperado), "faltou {esperado} em: {args}");
        }
        // Nada de `--disable`: um nome que o CLI nao conheca derrubaria o disparo.
        assert!(!args.contains("--disable"), "usou --disable: {args}");
        assert!(
            paths.config_dir.join("auth.txt").is_file(),
            "o CLI nao achou o login no CODEX_HOME"
        );
        assert!(
            !pasta_do_disparo(&paths).exists(),
            "a copia do login ficou no disco"
        );
    }

    /// A recusa por limite de uso chega explicada (e e' reconhecida para a espera
    /// maior), e a copia do login tambem some na falha.
    #[cfg(target_os = "windows")]
    #[test]
    fn falha_do_disparo_traz_o_motivo_do_cli() {
        let (paths, chave, ajustes) = ambiente_com_cli_falso(
            "limite",
            "echo You've hit your usage limit. Try again later.\r\nexit /b 1\r\n",
        );

        let erro = abrir_sessao(&paths, &chave, &ajustes).expect_err("o disparo devia falhar");
        assert!(erro.contains("limite de uso"), "sem a explicacao: {erro}");
        assert!(falha_por_limite(&erro));

        // A conta e' resolvida pela chave na hora do envio: uma chave que nao esta'
        // mais conectada nao envia nada (nem com o login de outra conta).
        let erro = abrir_sessao(&paths, "codex:outra", &ajustes).expect_err("conta inexistente");
        assert!(erro.contains("não está mais conectada"), "{erro}");
        assert!(!pasta_do_disparo(&paths).exists());
    }

    #[test]
    fn falhas_conhecidas_sao_explicadas() {
        assert!(falha_por_limite(
            "You've hit your usage limit. Try again later."
        ));
        assert!(!falha_por_limite("401 Unauthorized"));
        assert!(
            explicar_falha("error: unexpected argument '--ephemeral' found".to_string())
                .contains("Atualize")
        );
        assert_eq!(explicar_falha("outro erro".to_string()), "outro erro");
    }
}
