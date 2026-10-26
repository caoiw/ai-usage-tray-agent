// Painel de configurações nativo (abas), renderizado na webview.
// Lê/grava o config.json + autostart pelos comandos IPC `get_settings` e
// `save_settings`. Antes era um formulário servido por HTTP no navegador.
import { invoke } from "@tauri-apps/api/core";
import { animaTrocaDeAba } from "./anima";
import { escapeHtml } from "./usage-format";

interface CodexConfig {
  habilitado: boolean;
  mostraNaTaskbarWindows: boolean;
}
interface SessaoAutoConfig {
  habilitado: boolean;
  /** `"automatico"` (assim que a janela expira) ou `"agendado"` (só nos horários). */
  modo: string;
  /** Horários do modo agendado, em `"HH:MM"` de hora local, válidos todos os dias. */
  horarios: string[];
  /** Só editável à mão no arquivo: o backend ignora o valor que a UI manda e mantém
   * o do disco. */
  caminhoCli: string;
}
interface ClaudeConfig {
  habilitado: boolean;
  mostraNaTaskbarWindows: boolean;
  sessaoAuto: SessaoAutoConfig;
}
interface BarraConfig {
  lado: string;
  deslocamento: number;
  tamanhoFonte: number;
  corFonte: string;
  formatoReset: string;
  janelas: string;
}
interface WidgetConfig {
  habilitado: boolean;
  mostraClaude: boolean;
  mostraCodex: boolean;
  fundo: string;
  sempreNaFrente: boolean;
  opacidade: number;
  janelas: string;
  formatoReset: string;
  modo: string;
}
interface ServerConfig {
  habilitado: boolean;
  host: string;
  porta: number;
  pin: string;
}
interface AppConfig {
  usuario: string;
  intervaloSegundos: number;
  loki: { url: string };
  providers: { codex: CodexConfig; claude: ClaudeConfig };
  barraTarefas: BarraConfig;
  widget: WidgetConfig;
  servidor: ServerConfig;
}
/** Resultado da última reabertura automática de sessão (memória do backend) e a
 * conta vigiada agora (calculada a cada leitura). */
interface SessaoAutoStatus {
  ultimaTentativaEm?: string | null;
  ultimoOk?: boolean | null;
  ultimoErro?: string | null;
  emExecucao?: boolean;
  /// Apelido (senão e-mail) da conta cuja janela de 5h decide o disparo.
  contaVigiada?: string | null;
  /// A conta vigiada é a do login do CLI (e não só a única do app).
  contaDoCli?: boolean;
  /// Por que a reabertura está parada, quando não há conta para vigiar.
  aviso?: string | null;
}
interface SettingsData {
  autostart: boolean;
  os: string;
  autostartLabel: string;
  appVersion: string;
  config: AppConfig;
  sessaoAutoStatus?: SessaoAutoStatus;
  /** Ajustes gerais da sessão automática do Codex. Moram no contas.json (o
   * config.json não pode ganhar campos: a versão anterior o regrava). */
  sessaoAutoCodex?: SessaoAutoConfig;
}
/** Sessão automática de uma conta do Codex (memória do backend), na lista de
 * contas. O `aviso` diz por que nada está sendo enviado agora (sem janela de 5h,
 * cota esgotada). */
type SessaoAutoContaStatus = Pick<SessaoAutoStatus, "ultimaTentativaEm" | "ultimoOk" | "ultimoErro" | "emExecucao" | "aviso">;
interface SaveSettings {
  config: AppConfig;
  autostart: boolean;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;

// Segmented control (radio group): le' a opcao marcada e marca uma opcao por valor.
const radioValue = (name: string): string | undefined =>
  document.querySelector<HTMLInputElement>(`input[name="${name}"]:checked`)?.value;
const setRadio = (name: string, value: string): void => {
  const el = document.querySelector<HTMLInputElement>(`input[name="${name}"][value="${value}"]`);
  if (el) el.checked = true;
};

function fillForm(data: SettingsData): void {
  const c = data.config;
  const codex = c.providers.codex;
  const claude = c.providers.claude;
  const barra = c.barraTarefas;
  const widget = c.widget;
  const servidor = c.servidor;

  $<HTMLInputElement>("set-usuario").value = c.usuario ?? "";
  $<HTMLInputElement>("set-intervalo").value = String(c.intervaloSegundos ?? 10);
  $<HTMLInputElement>("set-lokiUrl").value = c.loki?.url ?? "";

  $<HTMLInputElement>("set-codexHab").checked = codex.habilitado !== false;
  $<HTMLInputElement>("set-codexTaskbar").checked = codex.mostraNaTaskbarWindows !== false;

  $<HTMLInputElement>("set-claudeHab").checked = claude.habilitado !== false;
  $<HTMLInputElement>("set-claudeTaskbar").checked = claude.mostraNaTaskbarWindows !== false;
  syncCartoes();
  preencherSessaoAuto("claude", claude.sessaoAuto);
  syncSessaoAuto(data.sessaoAutoStatus);
  syncSessaoAutoCampos("codex");

  $<HTMLSelectElement>("set-barraLado").value = barra.lado === "esquerda" ? "esquerda" : "direita";
  $<HTMLInputElement>("set-barraDesloc").value = String(barra.deslocamento ?? 0);
  $<HTMLInputElement>("set-barraFonte").value = String(barra.tamanhoFonte ?? 9);
  $<HTMLInputElement>("set-barraCor").value = barra.corFonte ?? "auto";
  $<HTMLSelectElement>("set-barraFormatoReset").value = barra.formatoReset === "exato" ? "exato" : "restante";
  $<HTMLSelectElement>("set-barraJanelas").value = normJanelas(barra.janelas);
  syncColorPicker();

  $<HTMLInputElement>("set-wdgClaude").checked = widget?.mostraClaude !== false;
  $<HTMLInputElement>("set-wdgCodex").checked = widget?.mostraCodex !== false;
  syncCartoes();
  $<HTMLInputElement>("set-wdgTopo").checked = widget?.sempreNaFrente !== false;
  $<HTMLInputElement>("set-wdgFundo").value = widget?.fundo ?? "";
  $<HTMLSelectElement>("set-wdgJanelas").value = normJanelas(widget?.janelas);
  $<HTMLSelectElement>("set-wdgFormatoReset").value = normResetMode(widget?.formatoReset);
  setWdgModo(widget?.modo);
  $<HTMLInputElement>("set-wdgOpac").value = String(widget?.opacidade ?? 90);
  syncOpacLabel();

  $<HTMLInputElement>("set-srvHab").checked = !!servidor?.habilitado;
  $<HTMLSelectElement>("set-srvHost").value = servidor?.host === "0.0.0.0" ? "0.0.0.0" : "127.0.0.1";
  $<HTMLInputElement>("set-srvPorta").value = String(servidor?.porta ?? 8770);
  $<HTMLInputElement>("set-srvPin").value = servidor?.pin ?? "";
  syncServerPinHint();
  syncProviderHints();

  $<HTMLInputElement>("set-autostart").checked = !!data.autostart;
  if (data.autostartLabel) $("set-autostartLabel").textContent = data.autostartLabel;
}

function collect(): SaveSettings {
  let intervalo = parseInt($<HTMLInputElement>("set-intervalo").value, 10);
  if (!Number.isFinite(intervalo)) intervalo = 10;
  let deslocamento = parseInt($<HTMLInputElement>("set-barraDesloc").value, 10);
  if (!Number.isFinite(deslocamento)) deslocamento = 0;
  let fonte = parseInt($<HTMLInputElement>("set-barraFonte").value, 10);
  if (!Number.isFinite(fonte)) fonte = 9;
  let opacidade = parseInt($<HTMLInputElement>("set-wdgOpac").value, 10);
  if (!Number.isFinite(opacidade)) opacidade = 90;
  let srvPorta = parseInt($<HTMLInputElement>("set-srvPorta").value, 10);
  if (!Number.isFinite(srvPorta) || srvPorta < 1 || srvPorta > 65535) srvPorta = 8770;

  const config: AppConfig = {
    usuario: $<HTMLInputElement>("set-usuario").value.trim(),
    intervaloSegundos: intervalo,
    loki: { url: $<HTMLInputElement>("set-lokiUrl").value.trim() },
    providers: {
      codex: {
        habilitado: $<HTMLInputElement>("set-codexHab").checked,
        mostraNaTaskbarWindows: $<HTMLInputElement>("set-codexTaskbar").checked,
      },
      claude: {
        habilitado: $<HTMLInputElement>("set-claudeHab").checked,
        mostraNaTaskbarWindows: $<HTMLInputElement>("set-claudeTaskbar").checked,
        sessaoAuto: ajustesSessaoAuto("claude"),
      },
    },
    barraTarefas: {
      lado: $<HTMLSelectElement>("set-barraLado").value,
      deslocamento,
      tamanhoFonte: fonte,
      corFonte: $<HTMLInputElement>("set-barraCor").value.trim() || "auto",
      formatoReset: $<HTMLSelectElement>("set-barraFormatoReset").value,
      janelas: $<HTMLSelectElement>("set-barraJanelas").value,
    },
    widget: {
      // O widget aparece quando ao menos um provedor esta marcado; nao ha mais
      // um checkbox separado de "mostrar widget".
      habilitado: $<HTMLInputElement>("set-wdgClaude").checked || $<HTMLInputElement>("set-wdgCodex").checked,
      mostraClaude: $<HTMLInputElement>("set-wdgClaude").checked,
      mostraCodex: $<HTMLInputElement>("set-wdgCodex").checked,
      fundo: $<HTMLInputElement>("set-wdgFundo").value.trim(),
      sempreNaFrente: $<HTMLInputElement>("set-wdgTopo").checked,
      opacidade,
      janelas: $<HTMLSelectElement>("set-wdgJanelas").value,
      formatoReset: $<HTMLSelectElement>("set-wdgFormatoReset").value,
      modo: getWdgModo(),
    },
    servidor: {
      habilitado: $<HTMLInputElement>("set-srvHab").checked,
      host: $<HTMLSelectElement>("set-srvHost").value,
      porta: srvPorta,
      pin: $<HTMLInputElement>("set-srvPin").value.trim(),
    },
  };
  return { config, autostart: $<HTMLInputElement>("set-autostart").checked };
}

function syncColorPicker(): void {
  const v = $<HTMLInputElement>("set-barraCor").value.trim();
  if (/^#?[0-9a-fA-F]{6}$/.test(v)) {
    $<HTMLInputElement>("set-barraCorPicker").value = "#" + v.replace(/^#/, "");
  }
}

/// Reflete o valor do slider de opacidade no rótulo ao lado.
function syncOpacLabel(): void {
  $("set-wdgOpacVal").textContent = $<HTMLInputElement>("set-wdgOpac").value;
}

/// Normaliza a opção de janelas para um dos valores válidos do <select>.
function normJanelas(value: string | undefined): "ambos" | "sessao" | "semanal" {
  return value === "sessao" || value === "semanal" ? value : "ambos";
}

/// Normaliza o formato do reset para um valor do <select> (default "restante").
function normResetMode(value: string | undefined): "restante" | "exato" | "nenhum" {
  return value === "exato" || value === "nenhum" ? value : "restante";
}

/// Normaliza o modo de exibição do widget para um valor do <select>
/// (default "completo").
function normModo(value: string | undefined): "completo" | "minimo" | "anelduplo" {
  return value === "minimo" || value === "anelduplo" ? value : "completo";
}

/// Lê/grava o modo de exibição do widget pelo grupo de radios das miniaturas
/// (cada miniatura desenha o widget no respectivo modo e seleciona ao clicar).
function getWdgModo(): string {
  const checked = document.querySelector<HTMLInputElement>('input[name="wdgModo"]:checked');
  return normModo(checked?.value);
}
function setWdgModo(value: string | undefined): void {
  const mode = normModo(value);
  document.querySelectorAll<HTMLInputElement>('input[name="wdgModo"]').forEach((radio) => {
    radio.checked = radio.value === mode;
  });
}

/// Abre o seletor de arquivo nativo (no backend) e joga o caminho escolhido no
/// campo de fundo. Como o campo é alterado por código (não dispara "change"),
/// agenda o auto-save explicitamente.
async function pickBackground(): Promise<void> {
  try {
    const path = await invoke<string | null>("pick_widget_background");
    if (path) {
      $<HTMLInputElement>("set-wdgFundo").value = path;
      scheduleAutoSave();
    }
  } catch (e) {
    setMsg("Falha ao escolher arquivo: " + (e instanceof Error ? e.message : String(e)), "err");
  }
}

function setMsg(text: string, kind?: "ok" | "err"): void {
  // Só exibimos erros no topo — coisas que o usuário talvez não perceba (ex.: falha
  // ao salvar). Confirmações de ações que ele mesmo disparou (conectar/desconectar)
  // são redundantes e limpam a área em vez de exibir. A exceção é o auto-save, que
  // ninguém dispara conscientemente: ele avisa por setSaved().
  const node = document.getElementById("settings-msg");
  if (!node) return;
  cancelMsgFade(node);
  if (kind === "err" && text) {
    node.textContent = text;
    node.className = "msg err";
  } else {
    node.textContent = "";
    node.className = "msg";
  }
}

/// Quanto tempo o "Configuração salva" fica legível antes de começar a sumir, e a
/// duração do esmaecimento (casada com a transição do .msg no styles.css).
const MSG_HOLD_MS = 2500;
const MSG_FADE_MS = 300;

// Timers do aviso efêmero "Configuração salva": um esmaece, o outro limpa. Ficam
// no módulo para que um aviso novo (ou um erro) cancele o sumiço do anterior.
let msgFadeTimer: number | undefined;
let msgClearTimer: number | undefined;

/// Cancela o sumiço programado e tira o nó do estado esmaecido, para que a próxima
/// mensagem apareça opaca e pelo tempo inteiro.
function cancelMsgFade(node: HTMLElement): void {
  if (msgFadeTimer !== undefined) {
    clearTimeout(msgFadeTimer);
    msgFadeTimer = undefined;
  }
  if (msgClearTimer !== undefined) {
    clearTimeout(msgClearTimer);
    msgClearTimer = undefined;
  }
  node.classList.remove("is-fading");
}

/// Confirma a gravação do config no lado oposto ao título ("Configurações"). O
/// save é automático e silencioso: sem esse aviso o usuário não tem como saber que
/// a alteração foi para o disco. Some sozinho para não virar ruído permanente.
function setSaved(): void {
  const node = document.getElementById("settings-msg");
  if (!node) return;
  cancelMsgFade(node);
  node.textContent = "Configuração salva";
  node.className = "msg ok";
  msgFadeTimer = window.setTimeout(() => {
    msgFadeTimer = undefined;
    node.classList.add("is-fading");
    msgClearTimer = window.setTimeout(() => {
      msgClearTimer = undefined;
      node.textContent = "";
      node.className = "msg";
    }, MSG_FADE_MS);
  }, MSG_HOLD_MS);
}

// O bloco `envio` (Enviar ao Loki por provedor) NÃO faz parte do save_settings
// (o backend o preserva relendo do disco). Os toggles nas abas Codex/Claude leem
// de `get_envio_state` e gravam via `set_envio_provider`, à parte do auto-save.
interface EnvioToggles {
  claude: { enviar: boolean };
  codex: { enviar: boolean };
}

/// Reflete nos checkboxes "Enviar ao Loki" o estado atual de envio por provedor.
async function loadEnvioToggles(): Promise<void> {
  try {
    const st = await invoke<EnvioToggles>("get_envio_state");
    $<HTMLInputElement>("set-codexEnviar").checked = !!st.codex?.enviar;
    $<HTMLInputElement>("set-claudeEnviar").checked = !!st.claude?.enviar;
    syncCartoes();
  } catch {
    // transitório; mantém o estado atual dos checkboxes
  }
}

/// Persiste o envio de um provedor direto via set_envio_provider.
async function setEnvioProvider(ferramenta: "codex" | "claude", enviar: boolean): Promise<void> {
  try {
    await invoke("set_envio_provider", { ferramenta, enviar });
    setSaved();
  } catch (e) {
    setMsg("Falha ao salvar o envio do provedor: " + (e instanceof Error ? e.message : String(e)), "err");
  }
}

// Contas conectadas de cada provedor (abas Codex e Claude). O login é só pelo
// navegador, à parte do config.json: `codex_login`/`claude_login` adicionam uma
// conta, ou reconectam a existente quando é a mesma. A lista vem de
// `codex_auth_status`/`claude_auth_status`. A conta principal é a que o tray, a
// barra, o widget e o envio ao Loki usam.
type ProvedorConta = "codex" | "claude";

/// Espelha `contas::MAX_CONTAS_POR_PROVEDOR` (limite de design). O backend recusa a
/// conta a mais de qualquer forma; aqui só esconde o "Adicionar conta".
const MAX_CONTAS_POR_PROVEDOR = 2;
/// Espelha `contas::MAX_APELIDO` (caracteres). O backend corta o excedente de
/// qualquer forma; aqui o campo não deixa digitar além.
const MAX_APELIDO = 20;

interface ContaStatus {
  /// "<provedor>:<id>", estável entre reconexões.
  chave: string;
  principal: boolean;
  apelido: string | null;
  /// A conta aparece no widget (quando o provedor também está ligado lá).
  mostraNoWidget: boolean;
  /// É a conta que a barra de tarefas mostra no provedor (uma por provedor; a
  /// principal, sem escolha).
  naBarra: boolean;
  /// É a conta enviada ao Loki no provedor (uma por provedor; a principal, sem
  /// escolha).
  enviada: boolean;
  connected: boolean;
  needsReconnect: boolean;
  email: string | null;
  /** Só no Codex: a sessão automática está ligada nesta conta, e o status dela. */
  sessaoAuto?: boolean;
  sessaoAutoStatus?: SessaoAutoContaStatus;
}

interface ProvedorUi {
  nome: string;
  contas: ContaStatus[];
  /// Assinatura da última lista desenhada: o auto-refresh de 5s só redesenha quando
  /// algo mudou, para não tirar o foco do campo de apelido no meio da digitação.
  assinatura: string;
  /// Enquanto true, o auto-refresh não relê a lista (não atropela o "Aguardando…").
  loginEmAndamento: boolean;
  /// Status de uma conta que precisa reconectar (o motivo difere entre provedores).
  expirada: string;
}

const PROVEDORES: Record<ProvedorConta, ProvedorUi> = {
  codex: {
    nome: "Codex",
    contas: [],
    assinatura: "",
    loginEmAndamento: false,
    expirada: "Não foi possível renovar a sessão automaticamente. Reconecte.",
  },
  claude: {
    nome: "Claude",
    contas: [],
    assinatura: "",
    loginEmAndamento: false,
    expirada: "Sessão expirada. Reconecte para continuar a coleta.",
  },
};

/// Há ao menos uma conta que coleta (conectada e sem precisar reconectar)? Usado
/// pelos avisos de "sem credenciais".
function temContaAtiva(p: ProvedorConta): boolean {
  return PROVEDORES[p].contas.some((c) => c.connected && !c.needsReconnect);
}

function contaHtml(p: ProvedorConta, c: ContaStatus, varias: boolean): string {
  const ui = PROVEDORES[p];
  const ativa = c.connected && !c.needsReconnect;
  const status = !c.connected ? "Desconectada." : c.needsReconnect ? ui.expirada : "Conectada.";
  // Com uma conta só, "principal" não distingue nada: o selo e o botão aparecem a
  // partir da segunda.
  const selo = varias && c.principal ? ' <span class="conta-selo">Principal</span>' : "";
  const botao = (acao: string, texto: string): string =>
    `<button type="button" class="btn btn-sm" data-acao="${acao}">${texto}</button>`;
  return `<div class="conta" data-conta="${escapeHtml(c.chave)}">
    <div class="conta-info">
      <div class="conta-nome">${escapeHtml(c.email ?? "Conta sem e-mail")}${selo}</div>
      <div class="conta-status ${ativa ? "ok" : "warn"}">${status}</div>
    </div>
    <input type="text" class="conta-apelido" placeholder="Apelido" maxlength="${MAX_APELIDO}" value="${escapeHtml(c.apelido ?? "")}">
    <div class="conta-acoes">
      ${c.needsReconnect || !c.connected ? botao("reconectar", "Reconectar") : ""}
      ${varias && !c.principal ? botao("principal", "Tornar principal") : ""}
      ${botao("remover", "Remover")}
    </div>
  </div>`;
}

/// Desenha a lista de contas do provedor e ajusta o botão de login ("Conectar" sem
/// conta, "Adicionar conta" com ao menos uma, nenhum no limite) e o texto de status.
function renderContas(p: ProvedorConta): void {
  const ui = PROVEDORES[p];
  const varias = ui.contas.length > 1;
  const noLimite = ui.contas.length >= MAX_CONTAS_POR_PROVEDOR;
  $(`set-${p}Contas`).innerHTML = ui.contas.map((c) => contaHtml(p, c, varias)).join("");
  const loginBtn = $(`set-${p}Login`);
  loginBtn.textContent = ui.contas.length ? "Adicionar conta" : "Conectar com o navegador";
  loginBtn.hidden = ui.loginEmAndamento || noLimite;
  const statusEl = $(`set-${p}AuthStatus`);
  if (!ui.loginEmAndamento) {
    statusEl.textContent = noLimite
      ? `Limite de ${MAX_CONTAS_POR_PROVEDOR} contas. Remova uma para conectar outra.`
      : "Conecte sua conta para iniciar a coleta.";
    statusEl.hidden = ui.contas.length > 0 && !noLimite;
  }
  syncProviderHints();
  renderWidgetContas(p);
  renderEscolhaContas(p, "barra", (c) => c.naBarra);
  renderEscolhaContas(p, "envio", (c) => c.enviada);
  if (p === "codex") renderSessaoAutoContas();
}

/// Uma linha de status (texto e cor) da sessão automática.
type LinhaStatus = { texto: string; classe: "" | "ok" | "warn" };

/// Resultado da última reabertura, ou o envio em andamento. `null` sem tentativa.
function textoDaTentativa(st: SessaoAutoContaStatus): LinhaStatus | null {
  if (st.emExecucao) return { texto: "Enviando o “Oi”… (o CLI leva alguns segundos)", classe: "" };
  if (!st.ultimaTentativaEm) return null;
  const quando = new Date(st.ultimaTentativaEm).toLocaleString();
  if (st.ultimoOk) return { texto: `Última reabertura em ${quando}`, classe: "ok" };
  // O motivo pode faltar (falha sem mensagem do CLI); nesse caso não deixa um
  // traço solto no fim.
  return { texto: `Falha ao reabrir em ${quando}${st.ultimoErro ? ` - ${st.ultimoErro}` : ""}`, classe: "warn" };
}

/// Linha de status da sessão automática de uma conta do Codex: o envio em
/// andamento, o motivo de estar parada ou o resultado da última tentativa.
function statusSessaoAutoConta(c: ContaStatus): LinhaStatus {
  const st = c.sessaoAutoStatus ?? {};
  const tentativa = textoDaTentativa(st);
  if (st.emExecucao && tentativa) return tentativa;
  // Desligada, a conta ainda pode ter usado o "Enviar teste" (é o jeito de
  // conferir o CLI antes de ligar): o resultado dele aparece junto.
  if (!c.sessaoAuto) {
    return tentativa
      ? { texto: `Desligada nesta conta. ${tentativa.texto}`, classe: tentativa.classe }
      : { texto: "Desligada nesta conta.", classe: "" };
  }
  if (st.aviso) return { texto: st.aviso, classe: "warn" };
  return tentativa ?? { texto: "Aguardando a janela de 5h ficar livre.", classe: "" };
}

/// Contas do cartão "Reabrir a sessão automaticamente" da aba Codex: um switch por
/// conta (a função gasta cota da conta, então cada uma é ligada à parte), o status
/// e o "Enviar teste". Redesenhada junto com a lista de contas, que o auto-refresh
/// de 5s relê — é assim que o status acompanha o envio.
function renderSessaoAutoContas(): void {
  const contas = PROVEDORES.codex.contas.filter((c) => c.connected);
  const lista = $("set-codexSessaoAutoContas");
  $("set-codexSessaoAutoSemContas").hidden = contas.length > 0;
  // O esqueleto das linhas só é recriado quando as contas mudam: o auto-refresh
  // traz só o status novo, e recriar o switch e o botão debaixo do cursor faria um
  // clique no instante da troca se perder.
  const linhas = Array.from(lista.querySelectorAll<HTMLElement>(".conta"));
  const mesmasContas = linhas.length === contas.length && linhas.every((linha, i) => linha.dataset.conta === contas[i].chave);
  if (!mesmasContas) {
    lista.innerHTML = contas.map((c) => `<div class="conta" data-conta="${escapeHtml(c.chave)}">
      <div class="conta-info">
        <div class="conta-nome"></div>
        <div class="conta-status"></div>
      </div>
      <div class="conta-acoes">
        <button type="button" class="btn btn-sm" data-acao="testar">Enviar teste</button>
      </div>
      <label class="switch switch-sm" title="Reabrir a sessão automaticamente nesta conta">
        <input type="checkbox" data-conta="${escapeHtml(c.chave)}">
        <span class="switch-track"></span>
      </label>
    </div>`).join("");
  }
  const provedorLigado = $<HTMLInputElement>("set-codexHab").checked;
  lista.querySelectorAll<HTMLElement>(".conta").forEach((linha, i) => {
    const c = contas[i];
    const { texto, classe } = statusSessaoAutoConta(c);
    linha.querySelector(".conta-nome")!.textContent = rotuloConta(c);
    const status = linha.querySelector(".conta-status")!;
    status.textContent = texto;
    status.className = `conta-status ${classe}`;
    linha.querySelector<HTMLButtonElement>('button[data-acao="testar"]')!.disabled =
      !provedorLigado || !!c.sessaoAutoStatus?.emExecucao;
    const ligada = linha.querySelector<HTMLInputElement>("input[data-conta]")!;
    ligada.checked = !!c.sessaoAuto;
    ligada.disabled = !provedorLigado;
  });
}

/// "Enviar teste": manda o "Oi" agora na conta. O backend volta na hora; o
/// resultado chega pelo status da conta.
async function testarSessaoAuto(chave: string): Promise<void> {
  try {
    await invoke("testar_sessao_auto_codex", { conta: chave });
  } catch (e) {
    setMsg("Falha ao enviar o teste: " + (e instanceof Error ? e.message : String(e)), "err");
  }
  await loadContas("codex", true);
}

/// Rótulo curto da conta: o apelido, senão o e-mail.
function rotuloConta(c: ContaStatus): string {
  return c.apelido ?? c.email ?? "Conta sem e-mail";
}

/// Nome da conta nas listas das abas Widget, Barra e Envio: apelido, com o e-mail
/// embaixo (sem apelido, o e-mail é o nome).
function nomeDaConta(c: ContaStatus): string {
  const nome = rotuloConta(c);
  const email = c.apelido && c.email ? `<span class="prov-conta-email">${escapeHtml(c.email)}</span>` : "";
  return `<span class="prov-conta-textos"><span class="prov-conta-nome">${escapeHtml(nome)}</span>${email}</span>`;
}

/// Lista de contas do cartão de um provedor nas abas Widget/Barra/Envio. Só com 2
/// ou mais contas: com uma só, o switch do provedor já basta. `item` desenha o
/// controle de cada conta.
function renderListaContas(p: ProvedorConta, aba: "wdg" | "barra" | "envio", item: (c: ContaStatus) => string): void {
  const ui = PROVEDORES[p];
  const lista = $(`set-${aba}${ui.nome}Contas`);
  const varias = ui.contas.length > 1;
  lista.hidden = !varias;
  lista.innerHTML = varias
    ? ui.contas.map((c) => `<label class="prov-conta">${nomeDaConta(c)}${item(c)}</label>`).join("")
    : "";
}

/// Abas Barra e Envio: vai uma conta por provedor (a barra mostra uma; o Loki
/// recebe uma). O cartão lista as contas com um radio para escolher qual; sem
/// escolha, vem marcada a principal. Ligar o provedor continua no switch de cima.
function renderEscolhaContas(p: ProvedorConta, aba: "barra" | "envio", marcada: (c: ContaStatus) => boolean): void {
  renderListaContas(p, aba, (c) =>
    `<input type="radio" class="prov-conta-radio" name="${aba}-${p}" data-conta="${escapeHtml(c.chave)}"${marcada(c) ? " checked" : ""}>`);
}

/// Aba Widget: um switch por conta (qualquer combinação delas aparece).
function renderWidgetContas(p: ProvedorConta): void {
  renderListaContas(p, "wdg", (c) =>
    `<span class="switch switch-sm"><input type="checkbox" data-conta="${escapeHtml(c.chave)}"${c.mostraNoWidget !== false ? " checked" : ""}>` +
    `<span class="switch-track"></span></span>`);
}

/// Provedor desligado numa das abas Widget/Barra/Envio: as contas dele no cartão
/// ficam apagadas e sem clique.
function syncCartoes(): void {
  for (const nome of ["Claude", "Codex"]) {
    const n = nome.toLowerCase();
    const cartoes: [string, string][] = [["wdg", `set-wdg${nome}`], ["barra", `set-${n}Taskbar`], ["envio", `set-${n}Enviar`]];
    for (const [aba, idSwitch] of cartoes) {
      $(`set-${aba}${nome}Card`).classList.toggle("off", !$<HTMLInputElement>(idSwitch).checked);
    }
  }
}

/// Grava a escolha de conta de uma das abas (`set_conta_widget`/`_barra`/
/// `_enviada`), à parte do auto-save do formulário (que só cuida do config.json).
async function salvarContaDaAba(p: ProvedorConta, comando: string, args: Record<string, unknown>, falha: string): Promise<void> {
  try {
    await invoke(comando, args);
    setSaved();
  } catch (e) {
    setMsg(`${falha}: ` + (e instanceof Error ? e.message : String(e)), "err");
  }
  // Com erro, a lista redesenhada devolve o controle ao valor gravado.
  await loadContas(p, true);
}

/// Relê a lista de contas (sem rede). Sem `forcar`, só redesenha se mudou.
async function loadContas(p: ProvedorConta, forcar = false): Promise<void> {
  try {
    const contas = await invoke<ContaStatus[]>(`${p}_auth_status`);
    const assinatura = JSON.stringify(contas);
    const ui = PROVEDORES[p];
    if (!forcar && assinatura === ui.assinatura) return;
    ui.contas = contas;
    ui.assinatura = assinatura;
    renderContas(p);
  } catch {
    // transitório; mantém o estado atual
  }
}

/// Dispara o login pelo navegador (bloqueante no backend até o usuário concluir ou
/// cancelar). Enquanto aguarda, troca o botão de login pelo "Cancelar" (que no
/// Codex libera a porta 1455). No Claude, se a conta tiver mais de uma org, a
/// escolha acontece na própria janela de login (claude-org.html) e o comando só
/// volta depois dela.
async function contaLogin(p: ProvedorConta): Promise<void> {
  const ui = PROVEDORES[p];
  const cancelBtn = $(`set-${p}LoginCancel`);
  const statusEl = $(`set-${p}AuthStatus`);
  const outraConta = ui.contas.length > 0;
  const antes = new Set(ui.contas.map((c) => c.chave));
  let nova: string | null = null;
  ui.loginEmAndamento = true;
  $(`set-${p}Login`).hidden = true;
  cancelBtn.hidden = false;
  statusEl.hidden = false;
  // O navegador do sistema pode já estar logado na OpenAI. O backend pede a tela de
  // login mesmo assim, mas se ela não aparecer, a saída é sair da conta lá.
  statusEl.textContent =
    p === "codex" && outraConta
      ? "Aguardando o login no navegador… Se ele entrar direto na conta já conectada, saia dela no navegador e tente de novo."
      : "Aguardando o login no navegador…";
  try {
    const status = await invoke<{ chave?: string } | null>(`${p}_login`);
    setMsg(`${ui.nome} conectado.`, "ok");
    if (status?.chave && !antes.has(status.chave)) nova = status.chave;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    // Cancelamento é ação do usuário; não polui o topo com aviso.
    if (/cancel/i.test(msg)) setMsg("");
    else setMsg(`Falha no login do ${ui.nome}: ${msg}`, "err");
  } finally {
    ui.loginEmAndamento = false;
    cancelBtn.hidden = true;
    await loadContas(p, true);
  }
  const conta = ui.contas.find((c) => c.chave === nova);
  if (conta) await pedirApelido(p, conta);
}

/// Oferece o apelido de uma conta que acabou de ser ADICIONADA, num modal: no campo
/// da lista ele passava despercebido. É opcional ("Agora não", Esc ou clique fora
/// fecham sem gravar). Reconectar uma conta que já estava na lista não pergunta.
function pedirApelido(p: ProvedorConta, conta: ContaStatus): Promise<void> {
  return new Promise((resolve) => {
    const overlay = $("apelido-overlay");
    const form = $<HTMLFormElement>("apelido-form");
    const input = $<HTMLInputElement>("apelido-input");
    const salvar = $<HTMLButtonElement>("apelido-salvar");
    const pular = $<HTMLButtonElement>("apelido-pular");
    $("apelido-conta").innerHTML =
      `${PROVEDORES[p].nome} conectado como <b>${escapeHtml(conta.email ?? "conta sem e-mail")}</b>.`;
    input.maxLength = MAX_APELIDO;
    input.value = "";
    salvar.disabled = true;
    overlay.classList.remove("hide");
    input.focus();

    const onInput = (): void => { salvar.disabled = !input.value.trim(); };
    const fechar = (): void => {
      overlay.classList.add("hide");
      input.removeEventListener("input", onInput);
      form.removeEventListener("submit", onSubmit);
      pular.removeEventListener("click", fechar);
      overlay.removeEventListener("mousedown", onBackdrop);
      document.removeEventListener("keydown", onKey);
      resolve();
    };
    const onSubmit = (e: SubmitEvent): void => {
      e.preventDefault();
      const apelido = input.value.trim();
      if (!apelido) return;
      fechar();
      void salvarApelido(p, conta.chave, apelido);
    };
    const onBackdrop = (e: MouseEvent): void => { if (e.target === overlay) fechar(); };
    const onKey = (e: KeyboardEvent): void => { if (e.key === "Escape") fechar(); };
    input.addEventListener("input", onInput);
    form.addEventListener("submit", onSubmit);
    pular.addEventListener("click", fechar);
    overlay.addEventListener("mousedown", onBackdrop);
    document.addEventListener("keydown", onKey);
  });
}

/// Cancela um login em andamento (o contaLogin pendente rejeita e se recupera).
async function contaLoginCancel(p: ProvedorConta): Promise<void> {
  try {
    await invoke(`${p}_login_cancel`);
  } catch {
    // best-effort; o login pendente ainda expira sozinho no timeout
  }
}

async function tornarPrincipal(p: ProvedorConta, chave: string): Promise<void> {
  try {
    await invoke("set_conta_principal", { conta: chave });
    setSaved();
  } catch (e) {
    setMsg("Falha ao trocar a conta principal: " + (e instanceof Error ? e.message : String(e)), "err");
  }
  await loadContas(p, true);
}

async function removerConta(p: ProvedorConta, chave: string): Promise<void> {
  try {
    await invoke("remover_conta", { conta: chave });
    setMsg("Conta removida.", "ok");
  } catch (e) {
    setMsg("Falha ao remover a conta: " + (e instanceof Error ? e.message : String(e)), "err");
  }
  await loadContas(p, true);
}

async function salvarApelido(p: ProvedorConta, chave: string, apelido: string): Promise<void> {
  try {
    await invoke("set_conta_apelido", { conta: chave, apelido });
    setSaved();
  } catch (e) {
    setMsg("Falha ao salvar o apelido: " + (e instanceof Error ? e.message : String(e)), "err");
  }
  await loadContas(p);
}

/// Liga os eventos da lista de contas e dos botões de login de um provedor. A
/// lista é redesenhada por innerHTML, então os cliques são tratados por delegação.
function wireContas(p: ProvedorConta): void {
  $(`set-${p}Login`).addEventListener("click", () => void contaLogin(p));
  $(`set-${p}LoginCancel`).addEventListener("click", () => void contaLoginCancel(p));
  const lista = $(`set-${p}Contas`);
  lista.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLButtonElement>("button[data-acao]");
    const chave = btn?.closest<HTMLElement>(".conta")?.dataset.conta;
    if (!btn || !chave) return;
    const acao = btn.dataset.acao;
    if (acao === "reconectar") {
      void contaLogin(p);
    } else if (acao === "principal") {
      void tornarPrincipal(p, chave);
    } else if (acao === "remover") {
      // Remover apaga as credenciais (voltar exige novo login): pede um segundo
      // clique no mesmo botão, que volta ao normal sozinho se não vier.
      if (btn.dataset.confirmar !== "1") {
        btn.dataset.confirmar = "1";
        btn.textContent = "Confirmar remoção";
        btn.classList.add("danger");
        window.setTimeout(() => {
          delete btn.dataset.confirmar;
          btn.textContent = "Remover";
          btn.classList.remove("danger");
        }, 3000);
        return;
      }
      void removerConta(p, chave);
    }
  });
  // Controles das contas nas abas Widget (switch), Barra e Envio (radio): vão para o
  // contas.json, à parte do auto-save do formulário (que só cuida do config.json).
  const nome = PROVEDORES[p].nome;
  const naLista = (idLista: string, grava: (chave: string, input: HTMLInputElement) => void): void => {
    $(idLista).addEventListener("change", (e) => {
      const input = e.target as HTMLInputElement;
      const chave = input.dataset.conta;
      if (!chave) return;
      e.stopPropagation();
      grava(chave, input);
    });
  };
  naLista(`set-wdg${nome}Contas`, (chave, input) =>
    void salvarContaDaAba(p, "set_conta_widget", { conta: chave, mostra: input.checked }, "Falha ao salvar a conta do widget"));
  naLista(`set-barra${nome}Contas`, (chave, input) => {
    if (input.checked) void salvarContaDaAba(p, "set_conta_barra", { conta: chave }, "Falha ao escolher a conta da barra");
  });
  naLista(`set-envio${nome}Contas`, (chave, input) => {
    if (input.checked) void salvarContaDaAba(p, "set_conta_enviada", { conta: chave }, "Falha ao escolher a conta enviada");
  });
  // O apelido é gravado à parte do auto-save do formulário (que só cuida do
  // config.json): stopPropagation evita o save_settings geral.
  lista.addEventListener("change", (e) => {
    const input = e.target as HTMLInputElement;
    if (!input.matches(".conta-apelido")) return;
    e.stopPropagation();
    const chave = input.closest<HTMLElement>(".conta")?.dataset.conta;
    if (chave) void salvarApelido(p, chave, input.value);
  });
}

export async function loadSettings(): Promise<void> {
  setMsg("");
  try {
    const data = await invoke<SettingsData>("get_settings");
    // O cartão usa contas.json: só a carga inicial e seu próprio save o preenchem.
    preencherSessaoAuto("codex", data.sessaoAutoCodex);
    fillForm(data);
    void loadEnvioToggles();
    void loadContas("codex");
    void loadContas("claude");
    $("settings-loading").hidden = true;
    $("settings-form").hidden = false;
  } catch (e) {
    $("settings-loading").textContent = "Falha ao carregar configurações: " + (e instanceof Error ? e.message : String(e));
  }
}

let saveTimer: number | undefined;
// Cresce a cada agendamento; a resposta de um save só re-preenche o formulário
// se nenhuma mudança nova ocorreu nesse meio-tempo (evita sobrescrever o que o
// usuário acabou de alterar enquanto o save anterior estava em voo).
let saveSeq = 0;

/// Há um campo de texto/número em foco? Nesse caso o auto-save não deve
/// re-preencher o formulário (sobrescreveria o que está sendo digitado). Para
/// checkbox/select/range re-preencher é inofensivo (o valor já bate).
function isEditingField(): boolean {
  const a = document.activeElement as HTMLInputElement | null;
  if (!a || a.tagName !== "INPUT") return false;
  return a.type === "text" || a.type === "number" || a.type === "password";
}

/// Agenda um save com debounce: alterações em rajada (vários toggles, digitação)
/// são unificadas num único envio. Substitui o antigo botão "Salvar".
function scheduleAutoSave(): void {
  saveSeq++;
  if (saveTimer !== undefined) clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    saveTimer = undefined;
    void autoSave();
  }, 400);
}

async function autoSave(): Promise<void> {
  const seq = saveSeq;
  try {
    const data = await invoke<SettingsData>("save_settings", { settings: collect() });
    // Só reflete a normalização (clamp de intervalo/fonte, validação de cor) se
    // não houve mudança nova e nada está sendo digitado.
    if (seq === saveSeq && !isEditingField()) fillForm(data);
    setSaved();
  } catch (e) {
    setMsg("Erro ao salvar: " + (e instanceof Error ? e.message : String(e)), "err");
  }
}

/// Mostra o aviso de PIN obrigatório quando o servidor está habilitado mas sem
/// PIN — deixa claro que, sem PIN, ele não inicia.
function syncServerPinHint(): void {
  const habilitado = $<HTMLInputElement>("set-srvHab").checked;
  setBodyEnabled("set-srvBody", habilitado);
  const semPin = $<HTMLInputElement>("set-srvPin").value.trim() === "";
  ($("set-srvPinWarn") as HTMLElement).hidden = !(habilitado && semPin);
}

/// Habilita/desabilita (e esmaece) o corpo de campos de um provedor conforme ele
/// esteja ligado. Os valores continuam legíveis para o save; só não são editáveis.
function setBodyEnabled(bodyId: string, on: boolean): void {
  const body = $(bodyId);
  body.classList.toggle("is-off", !on);
  body.querySelectorAll("input, button").forEach((el) => {
    (el as HTMLInputElement | HTMLButtonElement).disabled = !on;
  });
}

/// Reflete o estado de cada provedor: desabilita os campos quando desligado. O
/// estado da conexão já aparece no status do bloco de login de cada um.
function syncProviderHints(): void {
  setBodyEnabled("set-codexBody", $<HTMLInputElement>("set-codexHab").checked);
  setBodyEnabled("set-claudeBody", $<HTMLInputElement>("set-claudeHab").checked);
  syncProviderNotes();
}

/// Último status conhecido da reabertura automática. Chega no `get_settings` e na
/// releitura de 5s (`get_sessao_auto_status`), então é preservado entre os `sync`
/// disparados pelo próprio toggle.
let lastSessaoAutoStatus: SessaoAutoStatus | undefined;

/// Horários do modo agendado ("HH:MM", ordenados), por provedor. Fonte da verdade
/// da lista enquanto o painel está aberto: os chips e o save leem daqui, porque
/// não há um campo de formulário que caiba uma lista.
const sessaoAutoHorarios: Record<ProvedorConta, string[]> = { claude: [], codex: [] };

const sessaoAutoModo = (p: ProvedorConta): "automatico" | "agendado" =>
  radioValue(`${p}SessaoAutoModo`) === "agendado" ? "agendado" : "automatico";

/// Preenche o cartão de sessão automática do provedor com os ajustes salvos.
function preencherSessaoAuto(p: ProvedorConta, ajustes: SessaoAutoConfig | undefined): void {
  $<HTMLInputElement>(`set-${p}SessaoAuto`).checked = !!ajustes?.habilitado;
  setRadio(`${p}SessaoAutoModo`, ajustes?.modo === "agendado" ? "agendado" : "automatico");
  sessaoAutoHorarios[p] = [...(ajustes?.horarios ?? [])];
  renderSessaoAutoHorarios(p);
}

/// Ajustes do cartão como vão para o backend.
function ajustesSessaoAuto(p: ProvedorConta): SessaoAutoConfig {
  return {
    habilitado: $<HTMLInputElement>(`set-${p}SessaoAuto`).checked,
    modo: sessaoAutoModo(p),
    horarios: sessaoAutoHorarios[p],
    // O caminho do CLI só é editado à mão no arquivo; o backend mantém o do disco.
    caminhoCli: "",
  };
}

/// Cresce a cada gravação da sessão automática do Codex: só a resposta da mais
/// recente re-preenche o cartão (mesma ideia do `saveSeq` do auto-save).
let sessaoAutoCodexSeq = 0;

/// Grava os ajustes do cartão. O Claude vai no auto-save do config.json; o Codex,
/// no contas.json (`set_sessao_auto_codex`).
async function salvarSessaoAuto(p: ProvedorConta): Promise<void> {
  if (p === "claude") {
    scheduleAutoSave();
    return;
  }
  const seq = ++sessaoAutoCodexSeq;
  try {
    const gravado = await invoke<SessaoAutoConfig>("set_sessao_auto_codex", { ajustes: ajustesSessaoAuto("codex") });
    // Só a resposta da edição mais recente pode normalizar o cartão.
    if (seq === sessaoAutoCodexSeq) {
      preencherSessaoAuto("codex", gravado);
      syncSessaoAutoCampos("codex");
    }
    setSaved();
  } catch (e) {
    setMsg("Falha ao salvar a sessão automática do Codex: " + (e instanceof Error ? e.message : String(e)), "err");
  }
}

/// Reflete os campos do cartão depois de uma mudança (no Claude, junto com o
/// status da reabertura).
function syncSessaoAutoDe(p: ProvedorConta): void {
  if (p === "claude") syncSessaoAuto();
  else syncSessaoAutoCampos("codex");
}

/// Minuto do dia de um "HH:MM" já normalizado (usado só para achar o próximo).
const horarioEmMinutos = (horario: string): number => {
  const [hora, minuto] = horario.split(":");
  return Number(hora) * 60 + Number(minuto);
};

/// "HH:MM" a partir do valor do `input[type=time]`, que pode vir vazio ou com
/// segundos. O backend revalida (e descarta o que não presta); aqui é só para o
/// chip não nascer torto.
function normHorario(valor: string): string | null {
  const partes = /^(\d{1,2}):([0-5]\d)/.exec(valor.trim());
  if (!partes) return null;
  const hora = Number(partes[1]);
  if (hora > 23) return null;
  return `${String(hora).padStart(2, "0")}:${partes[2]}`;
}

/// Desenha um chip por horário salvo, cada um com o botão de remover. Recriado
/// inteiro a cada mudança: a lista é curta e assim não sobra listener velho.
function renderSessaoAutoHorarios(p: ProvedorConta): void {
  const lista = $(`set-${p}SessaoAutoLista`);
  lista.textContent = "";
  sessaoAutoHorarios[p].forEach((horario) => {
    const chip = document.createElement("span");
    chip.className = "horario";
    chip.append(horario);
    const remover = document.createElement("button");
    remover.type = "button";
    remover.textContent = "✕";
    remover.title = `Remover ${horario}`;
    remover.setAttribute("aria-label", `Remover ${horario}`);
    remover.addEventListener("click", () => {
      sessaoAutoHorarios[p] = sessaoAutoHorarios[p].filter((h) => h !== horario);
      renderSessaoAutoHorarios(p);
      syncSessaoAutoDe(p);
      void salvarSessaoAuto(p);
    });
    chip.append(remover);
    lista.append(chip);
  });
}

/// Adiciona o horário do campo à lista (ordenada, sem repetir) e salva. Ordenar
/// "HH:MM" como texto já dá a ordem cronológica (as horas são zero-padded).
function addSessaoAutoHorario(p: ProvedorConta): void {
  const input = $<HTMLInputElement>(`set-${p}SessaoAutoHora`);
  const horario = normHorario(input.value);
  if (!horario) return;
  input.value = "";
  if (sessaoAutoHorarios[p].includes(horario)) return;
  sessaoAutoHorarios[p] = [...sessaoAutoHorarios[p], horario].sort();
  renderSessaoAutoHorarios(p);
  syncSessaoAutoDe(p);
  void salvarSessaoAuto(p);
}

/// Explica o modo escolhido. No agendado, mostra qual é o próximo horário — é o
/// jeito mais rápido de o usuário confirmar que a lista faz o que ele espera.
function sessaoAutoModoDesc(p: ProvedorConta): string {
  const horarios = sessaoAutoHorarios[p];
  if (sessaoAutoModo(p) !== "agendado") {
    return "Assim que a janela de 5h expira, o app abre a próxima sozinho.";
  }
  if (horarios.length === 0) return "A sessão só é aberta nos horários que você escolher.";
  const agora = new Date();
  const minutosAgora = agora.getHours() * 60 + agora.getMinutes();
  const proximo = horarios.find((h) => horarioEmMinutos(h) > minutosAgora);
  return proximo
    ? `Próximo horário: hoje às ${proximo}.`
    : `Próximo horário: amanhã às ${horarios[0]}.`;
}

/// Mostra as opções de modo/horários só quando a função está ligada, e avisa do
/// agendado sem horário.
function syncSessaoAutoCampos(p: ProvedorConta): void {
  const ligado = $<HTMLInputElement>(`set-${p}SessaoAuto`).checked;
  const agendado = sessaoAutoModo(p) === "agendado";
  ($(`set-${p}SessaoAutoBody`) as HTMLElement).hidden = !ligado;
  ($(`set-${p}SessaoAutoHorariosField`) as HTMLElement).hidden = !agendado;
  // Agendado sem horário nenhum não abre nada: avisa em vez de fingir que está
  // funcionando.
  ($(`set-${p}SessaoAutoWarn`) as HTMLElement).hidden =
    !(ligado && agendado && sessaoAutoHorarios[p].length === 0);
  $(`set-${p}SessaoAutoModoDesc`).textContent = sessaoAutoModoDesc(p);
}

/// Relê o status da reabertura automática do Claude (última tentativa e conta
/// vigiada) sem recarregar o formulário inteiro.
async function loadSessaoAutoStatus(): Promise<void> {
  try {
    syncSessaoAuto(await invoke<SessaoAutoStatus>("get_sessao_auto_status"));
  } catch {
    // transitório; mantém o último status
  }
}

/// Reabertura do Claude: os campos do cartão, a conta vigiada e o resultado da
/// última tentativa abaixo do toggle — é onde o usuário descobre que o CLI não
/// está instalado ou que o login dele expirou.
function syncSessaoAuto(status?: SessaoAutoStatus): void {
  if (status !== undefined) lastSessaoAutoStatus = status;
  syncSessaoAutoCampos("claude");

  const st = lastSessaoAutoStatus;
  // Qual conta decide o disparo: a do login do CLI, que é onde o `claude -p` abre
  // a janela. Sem ela, o aviso diz por que a reabertura está parada.
  const conta = $("set-claudeSessaoAutoConta") as HTMLElement;
  conta.classList.toggle("warn", !!st?.aviso);
  if (st?.aviso) {
    conta.textContent = st.aviso;
  } else if (st?.contaVigiada) {
    conta.textContent = st.contaDoCli
      ? `Vigiando a conta ${st.contaVigiada}, a mesma em que o CLI está logado.`
      : `Vigiando a conta ${st.contaVigiada}.`;
  } else {
    conta.textContent = "";
  }
  conta.hidden = conta.textContent === "";

  const el = $("set-claudeSessaoAutoStatus") as HTMLElement;
  const tentativa = st ? textoDaTentativa(st) : null;
  el.hidden = !tentativa;
  el.textContent = tentativa?.texto ?? "";
  el.classList.remove("ok", "warn");
  if (tentativa?.classe) el.classList.add(tentativa.classe);
}

/// Aviso por provedor (abas Envio, Barra de tarefas e Widget): se o provedor está
/// desativado ou sem credenciais, avisa — mas o toggle segue operável.
function providerNote(habilitado: boolean, configurado: boolean): string {
  if (!habilitado) return "Provedor desativado";
  if (!configurado) return "Sem credenciais";
  return "";
}
function setNotes(ids: string[], msg: string): void {
  ids.forEach((id) => {
    const note = document.getElementById(id);
    if (!note) return;
    note.textContent = msg;
    (note as HTMLElement).hidden = msg === "";
  });
}
function syncProviderNotes(): void {
  const codexOn = $<HTMLInputElement>("set-codexHab").checked;
  const codexCfg = temContaAtiva("codex");
  const claudeOn = $<HTMLInputElement>("set-claudeHab").checked;
  const claudeCfg = temContaAtiva("claude");
  setNotes(["envio-codex-note"], providerNote(codexOn, codexCfg));
  setNotes(["envio-claude-note"], providerNote(claudeOn, claudeCfg));
  setNotes(["barra-codex-note", "wdg-codex-note"], providerNote(codexOn, codexCfg));
  setNotes(["barra-claude-note", "wdg-claude-note"], providerNote(claudeOn, claudeCfg));
}

/// Troca a aba levando o formulário da altura antiga até a nova e dando o fade de
/// entrada no painel escolhido (ver anima.ts) — sem isso o painel salta de
/// tamanho, já que cada aba tem uma quantidade diferente de campos.
function activateTab(tab: string): void {
  const painel = document.querySelector<HTMLElement>('.stab[data-spanel="' + tab + '"]');
  // Reclicar a aba que já está aberta não pode refazer nada: sem conteúdo novo
  // para revelar, o fade de entrada tocaria de novo e a aba piscaria à toa. Aqui
  // a aba atual mora no DOM (não há variável de estado), então quem responde é o
  // próprio painel.
  if (!painel || painel.classList.contains("on")) return;
  animaTrocaDeAba(
    $("settings-form"),
    () => {
      document.querySelectorAll(".settings-tabs button").forEach((b) =>
        b.classList.toggle("on", (b as HTMLElement).dataset.stab === tab));
      document.querySelectorAll(".stab").forEach((s) =>
        s.classList.toggle("on", (s as HTMLElement).dataset.spanel === tab));
    },
    painel,
  );
}

let initialized = false;

/// Liga os eventos da seção (uma vez) e carrega os valores atuais. Chamada na
/// primeira vez que o usuário abre a aba Configurações.
export function initSettings(): void {
  if (initialized) { void loadSettings(); return; }
  initialized = true;

  document.querySelectorAll(".settings-tabs button").forEach((b) =>
    b.addEventListener("click", () => activateTab((b as HTMLElement).dataset.stab ?? "geral")));

  $("set-srvPinToggle").addEventListener("click", () => {
    const input = $<HTMLInputElement>("set-srvPin");
    const show = input.type === "password";
    input.type = show ? "text" : "password";
    $("set-srvPinToggle").textContent = show ? "Ocultar" : "Mostrar";
  });
  $("set-srvHab").addEventListener("change", syncServerPinHint);
  $("set-srvPin").addEventListener("input", syncServerPinHint);
  $("set-codexHab").addEventListener("change", syncProviderHints);
  for (const id of ["set-wdgClaude", "set-wdgCodex", "set-claudeTaskbar", "set-codexTaskbar", "set-claudeEnviar", "set-codexEnviar"]) {
    $(id).addEventListener("change", syncCartoes);
  }
  wireContas("codex");
  $("set-claudeHab").addEventListener("change", syncProviderHints);
  wireContas("claude");
  // Ao ligar, já mostra a conta vigiada, sem esperar a releitura de 5s.
  $("set-claudeSessaoAuto").addEventListener("change", () => {
    syncSessaoAuto();
    void loadSessaoAutoStatus();
  });
  $("set-claudeSessaoAutoModo").addEventListener("change", () => syncSessaoAuto());
  for (const p of ["claude", "codex"] as const) {
    $(`set-${p}SessaoAutoAdd`).addEventListener("click", () => addSessaoAutoHorario(p));
    // Enter no campo de hora adiciona, em vez de nada acontecer.
    $(`set-${p}SessaoAutoHora`).addEventListener("keydown", (e) => {
      if ((e as KeyboardEvent).key !== "Enter") return;
      e.preventDefault();
      addSessaoAutoHorario(p);
    });
    // Escolher uma hora no campo não muda o config (só entra na lista pelo
    // "Adicionar"), então o "change" dele não deve chegar ao auto-save.
    $(`set-${p}SessaoAutoHora`).addEventListener("change", (e) => e.stopPropagation());
  }
  // Sessão automática do Codex: grava no contas.json (set_sessao_auto_codex e, por
  // conta, set_conta_sessao_auto), então nenhum "change" do cartão vai para o
  // auto-save do config.json.
  $("set-codexSessaoAutoCard").addEventListener("change", (e) => {
    e.stopPropagation();
    const input = e.target as HTMLInputElement;
    const chave = input.dataset.conta;
    if (chave) {
      void salvarContaDaAba("codex", "set_conta_sessao_auto", { conta: chave, ligado: input.checked }, "Falha ao salvar a conta da sessão automática");
      return;
    }
    syncSessaoAutoCampos("codex");
    void salvarSessaoAuto("codex");
  });
  $("set-codexSessaoAutoContas").addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-acao="testar"]');
    const chave = btn?.closest<HTMLElement>(".conta")?.dataset.conta;
    if (!btn || !chave) return;
    btn.disabled = true;
    void testarSessaoAuto(chave);
  });
  $("set-barraCor").addEventListener("input", syncColorPicker);
  $("set-barraCorPicker").addEventListener("input", () => {
    $<HTMLInputElement>("set-barraCor").value = $<HTMLInputElement>("set-barraCorPicker").value;
  });

  $("set-wdgOpac").addEventListener("input", syncOpacLabel);
  $("set-wdgFundoPick").addEventListener("click", () => void pickBackground());
  $("set-wdgFundoClear").addEventListener("click", () => {
    $<HTMLInputElement>("set-wdgFundo").value = "";
    scheduleAutoSave();
  });

  // "Enviar ao Loki" por provedor: persistido via set_envio_provider, à parte do
  // auto-save (stopPropagation evita o save_settings geral, que apenas preservaria
  // o bloco `envio` de qualquer forma).
  const wireEnvio = (id: string, ferramenta: "codex" | "claude"): void => {
    $(id).addEventListener("change", (e) => {
      e.stopPropagation();
      void setEnvioProvider(ferramenta, $<HTMLInputElement>(id).checked);
    });
  };
  wireEnvio("set-codexEnviar", "codex");
  wireEnvio("set-claudeEnviar", "claude");

  // Auto-save: qualquer alteração nos controles (toggle, select, slider, ou ao
  // sair de um campo de texto) persiste sozinha. O evento "change" borbulha, então
  // um único listener no formulário cobre todos os campos. Setar valores por
  // código (fillForm, picker de cor/fundo) não dispara "change", logo não há laço.
  $("settings-form").addEventListener("change", () => scheduleAutoSave());

  // Auto-refresh das contas: enquanto a tela Configurações está visível, relê a
  // lista a cada 5s para refletir mudanças externas (ex.: sessão/token expirou e a
  // coleta marcou "Reconectar"). Só redesenha a lista quando ela muda, sem tocar
  // nos campos; pula durante um login em andamento. A janela é destruída ao
  // fechar, então o timer não vaza entre reaberturas.
  window.setInterval(() => {
    const visivel = document.getElementById("view-settings")?.classList.contains("on");
    if (!visivel) return;
    for (const p of ["codex", "claude"] as const) {
      if (!PROVEDORES[p].loginEmAndamento) void loadContas(p);
    }
    // A conta do CLI muda por fora (`claude /login`): relê a conta vigiada.
    if ($<HTMLInputElement>("set-claudeSessaoAuto").checked) void loadSessaoAutoStatus();
  }, 5000);

  void loadSettings();
}
