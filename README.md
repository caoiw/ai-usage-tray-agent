# ai-usage-tray-agent

Aplicativo desktop cross-platform para Windows e Linux que roda no tray, coleta uso real de Codex e Claude e envia métricas para Grafana Loki.

O projeto foi feito com:

- Tauri v2
- Rust no backend
- Vite + TypeScript no frontend (UI nativa em webview)

## O que ele faz

- Inicia no tray sem abrir janela principal para o usuário
- Tem uma janela nativa (webview) com **Envio de dados**, **Uso atual**, **Dashboard Claude**, **Dashboard Codex**, **Configurações** e **Sobre** (versão, atualização e novidades)
- Coleta uso do Codex e do Claude em intervalo configurável
- Aceita **até duas contas por provedor** (ex.: trabalho e pessoal), cada uma com
  seu apelido
- Envia logs estruturados JSON para Loki
- Mantém logs locais
- Mostra status resumido no tray
- No Windows, exibe o uso direto na barra de tarefas
- No Windows, oferece um **widget flutuante na área de trabalho** com os cards de uso
- Permite pausar e retomar o envio pelo menu do tray
- Opcionalmente, **reabre sozinho a sessão (5h) do Claude** — quando ela expira ou
  em horários fixos que você escolhe

## Status atual

Funcional, com:

- Coleta real do Codex com login pelo navegador (OAuth)
- Coleta real do Claude com login pelo navegador (captura a sessão da claude.ai)
- Envio para Loki sem `tenant` e sem `basic auth`
- Janela do app com **Envio de dados**, **Uso atual**, **Dashboard Claude**, **Dashboard Codex**,
  **Configurações** (com abas e **auto-save**) e **Sobre** (versão, atualização e novidades)
- Widget na barra de tarefas (Windows) e **widget flutuante na área de trabalho**
- Empacotamento pronto:
  - Windows: instalador `.msi`
  - Linux: `AppImage`
- Atualização automática (OTA) das versões instaladas (Windows `.msi` e Linux
  `AppImage`)

## Configuração

O app cria e lê `config.json` automaticamente.

As edições no arquivo são aplicadas **em ~1s, sem reiniciar**: o app detecta a
mudança pelo `mtime` durante a espera entre coletas e reaplica a configuração na
hora (posição/fonte/cor da barra, lado, visibilidade dos provedores e o próprio
`intervaloSegundos`). Isso **não** dispara um envio extra ao Loki — o intervalo
de envio é preservado.

Windows:

- Config: `%AppData%/AiUsageTrayAgent/config.json`
- Logs: `%LocalAppData%/AiUsageTrayAgent/logs/`

Linux:

- Config: `~/.config/ai-usage-tray-agent/config.json`
- Logs: `~/.local/state/ai-usage-tray-agent/logs/`

Contas: o login de cada provedor fica em arquivos do próprio app, na mesma pasta do
`config.json`. A conta **principal** fica em `claude-auth.json` / `codex-auth.json`, e a
segunda conta de cada provedor em `contas/claude/` / `contas/codex/`. As preferências por
conta (apelido, ordem dos cards, o que aparece no widget, a conta da barra, a conta
enviada ao Loki e a abertura automática da sessão do Codex) ficam em `contas.json`. O `config.json` não muda de formato por causa das
contas: uma versão anterior do app continua funcionando e enxerga só a principal de cada
provedor.

Exemplo:

```json
{
  "usuario": "usuario-exemplo",
  "intervaloSegundos": 10,
  "loki": {
    "url": "http://loki.exemplo.local:3100/loki/api/v1/push"
  },
  "providers": {
    "codex": {
      "habilitado": true,
      "mostraNaTaskbarWindows": true
    },
    "claude": {
      "habilitado": true,
      "mostraNaTaskbarWindows": true,
      "sessaoAuto": {
        "habilitado": false,
        "modo": "automatico",
        "horarios": [],
        "caminhoCli": ""
      }
    },
    "ordem": ["claude", "codex"]
  },
  "usoAtual": {
    "grafico": true,
    "avisarAoDesligar": true
  },
  "barraTarefas": {
    "lado": "direita",
    "deslocamento": 0,
    "tamanhoFonte": 9,
    "corFonte": "auto",
    "formatoReset": "restante",
    "janelas": "ambos"
  },
  "widget": {
    "habilitado": false,
    "mostraClaude": true,
    "mostraCodex": true,
    "sempreNaFrente": true,
    "opacidade": 90,
    "janelas": "ambos",
    "formatoReset": "restante",
    "fundo": ""
  },
  "envio": {
    "pausado": false,
    "claude": true,
    "codex": true
  },
  "servidor": {
    "habilitado": false,
    "host": "127.0.0.1",
    "porta": 8770,
    "pin": ""
  }
}
```

> `janelas` (na barra e no widget) controla quais janelas aparecem: `"ambos"`
> (sessão 5h e semanal 7d), `"sessao"` (só 5h) ou `"semanal"` (só 7d).

## Formato enviado para o Loki

Labels:

- `app = "ai-usage-tray-agent"`
- `usuario`
- `ferramenta`
- `host`

Payload interno:

```json
{
  "uso_percentual": 47.0,
  "restante_percentual": 53.0,
  "status": "ok",
  "reset_em": "2026-05-06T17:00:41+00:00"
}
```

O timestamp do Loki é enviado em nanossegundos no campo `values`.

Vai **uma conta por provedor** (um stream por provedor): a escolhida na aba **Envio** das
Configurações ou, sem escolha, a principal. Uma conta sem limites de uso (ex.: Claude no
plano gratuito, cuja API não traz nenhuma janela) não é enviada.

## Tray (ícone na bandeja)

- **Clique esquerdo** no ícone: abre a janela do app (Dashboard/Configurações).
- **Clique direito** no ícone: abre o menu.

Itens do menu:

- **Abrir**: abre a janela do app (mesma ação do clique esquerdo).
- Abrir `config.json`
- Abrir pasta de logs
- Pausar/retomar envio
- Sair

> A exibição de cada IA na barra de tarefas e a inicialização automática não são
> itens do tray; são editadas nas **Configurações** do app (abas Barra de
> tarefas e Geral).

## Janela do app (Envio de dados, Uso atual, Dashboard Claude, Dashboard Codex, Configurações e Sobre)

A interface é uma janela nativa (webview do Tauri), aberta pelo clique esquerdo
no tray ou pelo item **Abrir**. Não usa navegador nem servidor HTTP local: o
frontend conversa com o backend Rust por comandos (IPC). Um menu lateral troca
entre as seções. Fechar pela janela (X) recolhe o app para o **tray** (a janela é
fechada, mas o app continua rodando); ao reabrir, ela volta no **tamanho e posição**
em que você a deixou (lembrados entre aberturas via `tauri-plugin-window-state`).

### Envio de dados

Primeira tela do menu. Controla o **envio das métricas ao Loki** sem afetar a
coleta — os dados continuam sendo coletados e exibidos em "Uso atual", no widget
e na barra mesmo com o envio pausado/desabilitado. Traz:

- O **subtítulo da página** traz a **contagem regressiva** do próximo envio
  automático (mesmo padrão do "Atualizado há…" do "Uso atual"); some quando o
  envio está pausado ou sem provedor ativo.
- **Estado atual** com um indicador "ao vivo" (ponto pulsante quando ativo) e o
  **status de envio de cada provedor** (Claude/Codex: ativado/desativado; com duas
  contas no provedor, também qual delas está sendo enviada). Quando
  **nenhum** provedor está com "Enviar ao Loki" ativado, o indicador troca para
  **"Nenhum provedor enviando"** (em vez de "Envio ativo") e o histórico exibe um
  lembrete para ativar Claude ou Codex em Configurações.
- **Pausar/retomar envio** (geral): suspende só o envio ao Loki. Persistido em
  `config.json` (`envio.pausado`) e **sincronizado com o menu do tray** — pausar
  pelo tray reflete aqui e vice-versa.
- **Histórico de envios**: data/hora e status (sucesso/falha) dos últimos envios,
  atualizado quase em tempo real; cada **nova entrada pisca** ao aparecer. Nos
  envios com sucesso, a entrada mostra os **dados enviados** ao Loki (uso da
  sessão 5h, uso semanal 7d e o reset), com o **payload completo** disponível no
  tooltip da linha. Botão **Limpar** zera a lista.

O liga/desliga do envio **por provedor** (`envio.claude`, `envio.codex`) fica nas
**Configurações**, na aba **Envio** (opção "Enviar ao Loki" por provedor), junto com a
escolha da **conta enviada** quando o provedor tem duas (`set_conta_enviada`, gravada no
`contas.json`). Tudo em
`envio` é **persistido no `config.json`**; a pausa geral é gerenciada por esta
tela e o painel de **Configurações** preserva esse bloco (editar configurações
não reativa o envio nem tira a pausa). O histórico tem altura limitada e **rola
por dentro** do card.

Os dados vêm do comando `get_envio_state`; as ações usam `set_envio_paused`,
`set_envio_provider` (a partir das Configurações) e `clear_send_log`.

### Uso atual

Um card por provedor (**Claude** e **Codex**), com o apelido da conta (senão o e-mail)
do lado oposto ao nome. Cada card mostra o uso da **sessão
(5h)** e **semanal (7d)** com barra de progresso, tempo restante para o reset
(contagem regressiva ao vivo) e o horário/data exatos do próximo reset — os dois
numa linha só, separados por um ponto, com o que não couber cortado por
reticências. Na janela semanal a data vira **"hoje"** ou **"amanhã"** quando o
reset cai num desses dias (a conta é por virada de meia-noite, não por 24h). O
subtítulo da página traz o **"Atualizado há Xs"** do dado em cache (sobe ao vivo
e zera a cada nova coleta). Se um provedor informa só uma das janelas (ex.: o
Codex só com o semanal), o bloco vazio é omitido e a outra ocupa o card inteiro.
Com as duas janelas, elas dividem um container só, com um divisor no meio. Uma conta
sem limites de uso (ex.: Claude gratuito) mostra "Sem limites de uso para acompanhar".
Os dados vêm do comando `get_usage` (lê o mesmo
snapshot do tray/barra, sem rede).

Com **duas contas** no provedor, o card mostra uma completa e a outra **resumida** numa
coluna estreita ao lado (no formato de anéis do widget: a % de cada janela e, com o
gráfico ligado, o tempo até o reset). Ao abrir, a principal fica completa, à esquerda. As
setas **‹ Apelido ›** do cabeçalho alternam: a coluna do resumo cresce até a conta
completa e a outra encolhe até virar resumo, com animação. Uma conta com erro fica num
container próprio, com a mensagem em vermelho.

Abaixo de cada janela há um **mini gráfico de linha** com a evolução da
porcentagem de uso ao longo das **últimas ~5 horas** (passe o mouse para ver o
horário e o valor de cada ponto). O histórico é mantido **só em memória** pelo
backend (um anel de amostras, uma por coleta, podado a 5h) e vai junto no
`get_usage`; por ser em memória, ele **zera quando o app é fechado** e começa a
preencher de novo nas próximas coletas — por isso a série semanal (7d), que varia
pouco em 5h, aparece quase plana nessa janela.

No cabeçalho da página, uma pílula reúne dois controles:

- **Gráfico** (interruptor): mostra/oculta os mini gráficos. Ao **desligar**, o
  backend **para de gravar e descarta** o histórico em memória (aparece um aviso de
  perda de dados, com "Não perguntar novamente"). É persistido em
  `providers`/`usoAtual.grafico` no `config.json` (gravado pelo comando
  `set_usage_chart`, fora do auto-save das Configurações).
- **Reordenar**: entra no modo de reordenação — os cards ganham uma alça e ficam
  **arrastáveis**; arraste um sobre o outro para trocar a ordem (cada provedor leva as
  contas dele junto). A ordem é gravada no `contas.json` e espelhada em
  `providers.ordem` do `config.json` (via `set_contas_ordem`), e vale também para o
  **widget** e a **barra de tarefas**.

Os dados vêm do comando `get_usage` (lê o mesmo snapshot do tray/barra, sem rede).

### Dashboard Claude

Replica o painel de uso do Claude Code lendo as mesmas fontes locais
(`~/.claude/projects/**/*.jsonl` e `~/.claude/stats-cache.json`). Tem abas:
**Visão Geral** (cards de resumo + heatmap de atividade), **Modelos** (gráfico de
tokens por modelo), **Ferramentas** (ranking das ferramentas mais usadas) e
**Projetos** (uso por projeto). Nos rankings o rótulo ocupa **uma linha**, cortada
no meio quando não cabe (começo + fim à mostra, como no Finder do macOS): o corte
é feito pelo CSS a partir da largura disponível, então alargar a janela revela
mais caracteres, e o nome completo fica no tooltip. Há um seletor de período **30d/7d** ou
**personalizado** (intervalo de datas, limitado ao período com dados); o padrão é
**30d**. As abas Ferramentas e Projetos dependem dos transcripts vivos (~30 dias),
então só enxergam esse período. Os dados vêm do comando `get_stats` e são
recarregados ao reabrir a janela. São os dados do Claude Code **nesta máquina, de todas
as contas** (o subtítulo da tela diz isso): os arquivos locais não registram de qual
conta veio cada mensagem.

### Dashboard Codex

Mostra o **histórico diário de uso do Codex** (em % da cota), no mesmo estilo da
Dashboard Claude. Os dados vêm de uma chamada à API de analytics do backend do
ChatGPT (`/backend-api/wham/usage/daily-token-usage-breakdown`) usando o mesmo
`access_token` da coleta, da conta escolhida — por isso a tela tem latência de rede e
carrega ao abrir (e ao trocar o período ou a conta). Traz:

- **Seletor de conta** no cabeçalho, com duas contas do Codex: mostra o apelido e, no
  menu, cada conta com o e-mail embaixo. Por padrão, a principal.

- **Cards de resumo**: dias ativos, uso médio/dia, dia de pico, maior uso, origem
  e modelo predominantes.
- **Visão Geral**: gráfico de barras do uso total por dia.
- **Origens**: barras empilhadas por *product surface* (CLI, VS Code, Web,
  JetBrains, SDK, GitHub…), com legenda.
- **Modelos**: barras empilhadas por modelo (ex.: GPT-5.5), com legenda.
- Seletor de período **30d/7d** ou **personalizado** (intervalo de datas até os
  últimos 90 dias, limite da API); cada troca refaz a chamada.
- Indicador de carregamento (a tela depende de rede) e mensagem quando não há uso
  no período.

Os dados vêm do comando `get_codex_stats` (que faz a chamada de rede no backend
Rust e devolve também as contas do seletor). A unidade é percentual de uso diário (não
tokens absolutos).

### Configurações

Formulário com **abas** que cobre **todas as opções do `config.json`** (mais o
"iniciar com o sistema"):

- **Geral**: `intervaloSegundos` e **Iniciar com o sistema** (autostart) — o
  autostart não fica no `config.json`, é gerenciado pelo `tauri-plugin-autostart`.
- **Envio**: `usuario` (Nome de exibição), `loki.url` e o **Enviar ao Loki** por
  provedor (`envio.codex`, `envio.claude`, como interruptores) — avisa quando o
  provedor está desativado ou sem credenciais, mas o interruptor segue operável. Com
  duas contas no provedor, o cartão dele lista as contas para escolher **qual é
  enviada** (padrão: a principal).
- **Codex** e **Claude**: `habilitado` (interruptor de destaque com o logo) e a **lista
  de contas**, com até duas por provedor. Cada conta tem apelido (até 20 caracteres),
  status e os botões **Tornar principal**, **Reconectar** e **Remover** (este pede um
  segundo clique, em vermelho). **Adicionar conta** faz um novo login pelo navegador, e
  logo depois do login de uma conta nova um modal oferece o apelido (opcional).
  Reconectar a mesma conta atualiza a existente, sem duplicar. A **principal** é a que as
  telas mostram primeiro, a padrão da barra e do envio, e a única que uma versão anterior
  do app enxerga.
- **Codex**, login: OAuth pelo navegador (você entra na conta ChatGPT/OpenAI; os tokens
  ficam salvos no próprio app e são renovados sozinhos). Com uma conta já conectada, o
  login pede a tela de entrada da OpenAI, para você poder entrar com outra. Traz ainda
  a **Abertura automática da sessão**, ligada por conta — ver a seção abaixo.
- **Claude**, login: um botão abre a claude.ai para você entrar (inclusive com Google); a
  sessão e o Organization ID são capturados e salvos no app, com aviso para
  reconectar quando a sessão expira. Se a conta
  tiver mais de uma organização, a própria janela de login mostra um modal para
  escolher qual usar (com o uso atual de cada uma), pois a coleta é por organização. Os campos ficam esmaecidos
  quando o provedor está desativado. Traz ainda a **Reabertura automática de sessão**
  (`sessaoAuto`) — ver a seção abaixo.
- **Barra de tarefas** (Windows): exibir cada provedor na barra
  (`providers.<ia>.mostraNaTaskbarWindows`), `lado`, `deslocamento`,
  `tamanhoFonte`, `corFonte` (com seletor de cor), `formatoReset` (tempo
  restante ou hora/data exata) e `janelas` (quais janelas exibir). A barra mostra
  **uma conta por provedor**: com duas, o cartão do provedor lista as contas para
  escolher qual (padrão: a principal).
- **Widget**: exibe o widget da área de trabalho — aparece quando ao menos um
  provedor está marcado (`mostraClaude`/`mostraCodex`) — e configura o que ele
  mostra: `sempreNaFrente`, `modo` (Completo, Mínimo ou Anel duplo, escolhido por
  miniaturas), `janelas`, `formatoReset` (tempo restante, hora/data exata ou
  nenhum), imagem/gif de `fundo` (com seletor de arquivo) e `opacidade` do painel.
  Com duas contas no provedor, o cartão dele ganha um interruptor por conta, para
  escolher quais aparecem no widget.
- **Servidor**: liga o **servidor HTTP dos dashboards** (ver seção abaixo) —
  `habilitado`, `host` (apenas local `127.0.0.1` ou rede `0.0.0.0`), `porta` e
  `pin` de acesso obrigatório (com mostrar/ocultar).

Não há botão "Salvar": as alterações têm **auto-save** (com debounce) — qualquer
mudança grava o `config.json` sozinha (com normalização: clamp de intervalo/fonte,
validação de cor) e o app aplica tudo em ~1s, **sem reiniciar e sem disparar um
envio extra** ao Loki. Cada gravação bem-sucedida mostra um **"Configuração salva"**
no canto oposto ao título da tela, que some sozinho em alguns segundos (no mesmo
lugar onde aparecem os erros de salvamento). O autostart é aplicado na hora. Os
valores são relidos do disco ao reabrir a tela. A opção **Enviar ao Loki** por
provedor (aba Envio) é gravada à parte (via `set_envio_provider`), fora do
auto-save, para preservar o bloco `envio` gerenciado pela tela **Envio de dados**.

### Sobre

Tela dedicada (última opção do menu lateral). Mostra:

- **Versão instalada** do app.
- **Atualização**: a verificação acontece **ao abrir o app**; havendo nova versão,
  o item **Sobre** do menu ganha o selo **Atualização disponível**. A tela Sobre
  reaproveita esse resultado (não verifica de novo ao abrir). Havendo update,
  mostra **Atualizar agora** (abre a janela de novidades, com o *delta* + barra de
  progresso); senão, **Buscar atualizações** força uma nova verificação inline.
- **Link do repositório** (abre no navegador via `open_external`).
- **Novidades**: o histórico de versões lido do `CHANGELOG.md` (via
  `get_changelog`), ocupando **o resto da altura da janela** (só a lista rola, e a
  página não ganha uma segunda barra de rolagem).

## Inicialização automática

O app usa o `tauri-plugin-autostart` (chave `HKCU\...\Run` no Windows) e vem
**habilitado por padrão na primeira execução**. Depois disso:

- O estado é controlado pela opção **Iniciar com o sistema** na aba **Geral**
  das Configurações.
- Se continuar ligado, o caminho do executável é reaplicado a cada início
  (evita apontar para um caminho antigo após atualizar/reinstalar).
- Se o usuário desligar pelas Configurações, permanece desligado nas próximas
  execuções.

## Reabertura automática da sessão do Claude

A cota da assinatura é contada em **janelas de 5h que só começam quando você manda
a primeira mensagem** — enquanto não há janela aberta, a API devolve
`five_hour.resets_at: null`. Quem quer aproveitar o dia inteiro precisa lembrar de
abrir a janela na mão (um "Oi") toda vez que a anterior expira. Ligando esta opção,
o app faz isso sozinho.

Fica na aba **Claude** das **Configurações** (ou em
`providers.claude.sessaoAuto.habilitado` no `config.json`) e **vem desligada** — o
recurso age na sua conta e consome cota, então só roda com opt-in explícito.

Como funciona:

- A cada coleta, se o Claude responder que **não há janela ativa**, o app dispara
  `claude -p "Oi"`. A mensagem é fixa; o objetivo é só carimbar o início da janela.
- A janela vigiada é a da **conta em que o CLI está logado** (lida do `.claude.json`
  do CLI: organização + e-mail), e não necessariamente a principal do app: é nessa
  conta que o `claude -p` abre a sessão. A seção mostra qual conta está sendo
  vigiada. Se o CLI estiver numa conta que não está conectada no app, **não dispara**
  e avisa (disparar ali abriria a janela de outra conta, gastando cota à toa); se não
  der para ler a conta do CLI, usa a única conta do app ou, com duas, não dispara.
- Só age sobre uma coleta **bem-sucedida** que diga isso explicitamente. Erro de
  coleta ou sessão expirada **não** contam como "janela fechada" — do contrário o
  app gastaria cota à toa, possivelmente em looping.
- Há **dois modos** (`providers.claude.sessaoAuto.modo`), escolhidos no seletor
  *Quando reabrir*:
  - **Assim que expira** (`"automatico"`, padrão): dispara na primeira coleta que
    encontrar a janela fechada, a qualquer hora do dia ou da noite.
  - **Em horários fixos** (`"agendado"`): dispara só nos horários de
    `providers.claude.sessaoAuto.horarios` — uma lista de `"HH:MM"` em **hora
    local**, válida todos os dias (ex.: `["09:00", "14:00", "19:00"]`). Útil para
    concentrar as janelas no horário em que você realmente trabalha, em vez de
    abrir uma às 3h da manhã. Sem nenhum horário na lista, nada é reaberto.
- No modo agendado o disparo é **no horário**, não depois dele: se naquele momento
  o app estava fechado, ou já havia uma janela de 5h em andamento, aquele horário é
  perdido e o app espera o próximo. Como quem decide é o ciclo de coleta, o horário
  vale com uma folga de um ciclo + 1 min (mínimo de 2 min) para não passar em branco.
- Há um intervalo mínimo de 3 min entre tentativas e um teto de 2 min por chamada
  (passou disso, o processo é encerrado e a tentativa vira falha).
- A chamada roda em segundo plano, sem segurar o ciclo de coleta — o tray, a barra
  e o widget continuam atualizando normalmente.
- Abaixo do interruptor, uma linha mostra o resultado da última tentativa (verde no
  sucesso, âmbar na falha, com o motivo) — é onde aparece, por exemplo, que o CLI
  não está instalado ou que o login dele expirou.
- O motivo vem do próprio CLI: o app captura o que ele escreve (o `claude -p`
  reporta a falha no **stdout**, com o stderr vazio), guarda os últimos ~300
  caracteres e usa isso tanto na linha de status quanto na entrada de erro do log
  (`logs/AAAA-MM-DD.log`, campo `meta.error`). Erros idênticos repetidos só vão ao
  log quando a mensagem muda, para não encher o arquivo.

Requisitos e detalhes:

- Precisa do **Claude Code CLI** instalado e logado **com a sua assinatura**
  (`npm i -g @anthropic-ai/claude-code`). O app procura o executável nos locais
  padrão de instalação e, se não achar, no `PATH`; para instalações fora desses
  caminhos, informe o caminho em `providers.claude.sessaoAuto.caminhoCli`.
- Uma variável `ANTHROPIC_API_KEY` (ou `ANTHROPIC_AUTH_TOKEN`) no ambiente é
  **removida** do processo filho: com ela, o CLI iria para a API paga, que é outro
  pool de cota e **não** abriria janela nenhuma.
- Cada disparo consome muito pouco (poucos tokens de entrada e saída), mas não é
  zero — ele aparece no uso que o próprio app exibe.
- O Claude Code grava um transcript a cada execução. Como esses "Oi" não são
  conversa de verdade, o app **apaga o transcript logo depois** — eles não poluem o
  histórico do CLI (`claude --resume`) nem a **Dashboard Claude**. Em troca, o uso
  desses disparos não é contabilizado no dashboard (que lê justamente esses
  arquivos), embora continue aparecendo no **Uso atual**, que vem do servidor.
- A limpeza é conservadora: o CLI roda num diretório dedicado
  (`sessao-auto`, dentro da pasta de configuração), e só é removida a pasta de
  transcripts cujos arquivos declaram esse diretório. Nenhum projeto seu é tocado.
- O Codex tem um recurso equivalente, com algumas diferenças: ver a seção abaixo.

## Reabertura automática da sessão do Codex

O mesmo recurso para o Codex: a janela de 5h do Codex também só começa na primeira
mensagem depois que a anterior acaba, e o app manda o "Oi" sozinho quando ela fica livre.

Fica na aba **Codex** das **Configurações**, no cartão *Reabrir a sessão
automaticamente*, e **vem desligado**. Há um interruptor geral, o mesmo seletor de
modo do Claude (*Assim que expira* ou *Em horários fixos*, com as mesmas regras de
horário) e a **lista de contas**: cada conta é ligada à parte, porque o recurso gasta
um pouco da cota de cada uma. Os ajustes ficam no `contas.json` (`sessaoAutoCodex` e
`sessaoAuto` por conta), e não no `config.json`.

Como funciona:

- O disparo é `codex exec "Oi"` pelo **Codex CLI**, mas **na conta do app**, não na
  conta em que o CLI está logado: a cada disparo o app cria uma pasta própria
  (`sessao-auto-codex`, dentro da pasta de configuração), copia para lá o login da
  conta e aponta o `CODEX_HOME` do CLI para ela. Assim o "Oi" cai sempre na conta
  certa, e nada é gravado no seu `~/.codex`. A pasta, com a cópia do login, é
  apagada ao fim de cada disparo — inclusive quando ele falha.
- **Janela parada**: o Codex não devolve "sem janela" como o Claude. A janela parada
  aparece com uso em 0% e o tempo até o reset igual à duração inteira (ela "anda"
  com o relógio enquanto ninguém a abre), ou com o reset já vencido. É isso que o app
  procura. A duração vem da própria API, então uma janela de outro tamanho que não 5h
  também funciona.
- **Sem janela de 5h, fica dormente**: se a OpenAI suspender o limite de 5h de novo,
  ou se o plano não tiver esse limite, nada é enviado, e a conta mostra o motivo.
  Quando a janela volta, o recurso volta sozinho.
- **Cota esgotada**: com a cota semanal (ou outro limite) esgotada, nada é enviado — um
  "Oi" ali não abriria nada.
- **Contra cobrança por API**: só vai login ChatGPT (assinatura). Uma conta com chave
  de API gravada é recusada, e `OPENAI_API_KEY`, `CODEX_API_KEY` e `OPENAI_BASE_URL`
  são retiradas do processo do CLI.
- **Modelo do CLI**: assim como no Claude, o app não fixa um modelo. A cada disparo,
  lê somente o campo global `model` de `$CODEX_HOME/config.toml` (ou `~/.codex/config.toml`
  quando a variável está ausente ou vazia) e o leva para a pasta isolada. Sem arquivo ou
  sem esse campo, o CLI usa seu próprio padrão. Configuração ilegível, inválida ou
  modelo vazio gera erro sem enviar nada; um modelo indisponível também aparece como
  falha, sem tentar outro automaticamente. Perfis e escolhas feitas só para uma sessão
  do terminal não são herdados. Nenhum arquivo pessoal é alterado.
- **Contexto reduzido**: mantém esforço `low`, instrução de sistema de uma linha e
  desativa skills, plugins, ferramentas, busca na web e instruções extras. Roda numa
  pasta vazia, com sandbox só de leitura. O consumo depende do modelo escolhido, e a
  abertura da janela de 5h depende de ele contar nessa cota. Outras configurações
  pessoais, como endpoints e plugins, não são copiadas.
- **O login da conta continua só com o app**: o app renova o token da conta antes do
  disparo, e a cópia entregue ao CLI vai **sem** o token de renovação. Assim o CLI
  usa o login, mas não tem como renová-lo por conta própria — o que invalidaria o
  token guardado no app e faria a coleta perder o login.
- **Esperas**: 15 min depois de um disparo que deu certo (a API demora a mostrar a
  janela nova), 5 min depois de uma falha e 30 min depois de uma recusa por limite de
  uso. O teto de cada chamada é de 2 min. Com várias contas, um disparo por vez.
  Desligar o provedor, o interruptor geral ou uma conta cancela os envios automáticos
  dela que ainda estejam aguardando na fila; um envio já iniciado segue até terminar.
- **Enviar teste**: manda o "Oi" na hora, sem olhar a janela (mas ainda com a proteção
  contra cobrança). Serve para ver se o CLI funciona; se houver uma janela correndo,
  ele só gasta um pouco dela.
- A linha de cada conta mostra o envio em andamento, o motivo de estar parada ou o
  resultado da última tentativa (o detalhe da falha também vai para o log).

Requisitos:

- **Codex CLI** instalado (`npm i -g @openai/codex`). O login dele **não** é usado. O app
  procura o executável nos locais padrão (inclusive o npm global do `fnm`) e, se não
  achar, no `PATH`; para instalações fora desses caminhos, informe o caminho em
  `sessaoAutoCodex.caminhoCli` no `contas.json`.

## Barra de tarefas (Windows)

No Windows o app desenha o uso diretamente na barra de tarefas. Cada provedor
**habilitado** vira um elemento separado, com duas linhas: o nome e, abaixo,
`uso da sessão (5h)` e `uso semanal (7d)`, cada um com o tempo até o reset:

```text
        Claude
61% (3:33h) | 40% (5d)
```

- O primeiro valor é o uso da janela de 5h e quanto falta para resetar.
- O segundo valor é o uso dos últimos 7 dias e quanto falta para resetar.
- Um **clique** no widget abre a janela do app (igual ao clique esquerdo no tray).
- Provedores com `"habilitado": false` no `config.json` não aparecem na barra.
- A exibição de cada provedor na barra é controlada por
  `providers.<ia>.mostraNaTaskbarWindows` (padrão `true`). Você pode alterar isso
  pelas **Configurações** do app ou editando o `config.json` direto; nos dois
  casos vale em ~1s. Só aparece na barra quando `habilitado` **e**
  `mostraNaTaskbarWindows` forem `true`.
- A barra mostra **uma conta por provedor**: com duas, a escolhida na aba Barra das
  Configurações (gravada no `contas.json`); sem escolha, a principal.
- Em Linux/macOS o campo `mostraNaTaskbarWindows` é lido mas **ignorado**: o
  widget da barra de tarefas só existe no Windows. O campo é mantido no arquivo
  para que a mesma `config.json` seja portável entre sistemas.

Como funciona:

- A Microsoft removeu o suporte a *deskbands* na barra de tarefas reescrita do
  Windows 11, então o texto não é uma deskband COM clássica.
- Em vez disso, o app cria uma pequena janela por provedor e a torna *filha* da
  janela da barra (`Shell_TrayWnd`) via Win32 `SetParent`. O texto fica de fato
  dentro da barra.
- A cor do texto é escolhida automaticamente pela cor real da barra
  (tema claro/escuro e *accent color*), para manter contraste.
- A janela é reposicionada periodicamente e recriada sozinha se o Explorer
  reiniciar.

Posicionamento:

- O lado é controlado por `config.json` → `barraTarefas.lado`: `"direita"`
  (padrão) ou `"esquerda"`. O cálculo que "adivinha" a posição é espelhado
  conforme o lado.
- **Direita** (padrão): o widget fica à esquerda da área de notificação
  (bandeja). Se houver outros widgets de terceiros embutidos na faixa direita da
  barra (monitores de rede, etc.), ele detecta e se ancora à esquerda deles,
  para conviverem sem sobreposição.
- **Esquerda**: o widget fica na ponta esquerda da barra, ancorado à direita de
  eventuais widgets ali (ex.: botão de Widgets/clima). É útil com o menu Iniciar
  **centralizado**, que deixa a ponta esquerda livre. Não é recomendado com a
  barra alinhada à esquerda (Iniciar/apps na esquerda), pois o espaço já está
  ocupado — nesse caso use o lado direito.
- Ajuste fino manual: `config.json` → `barraTarefas.deslocamento` (px). Negativo
  move o widget para a esquerda, positivo para a direita, **em ambos os lados**.
  Útil quando há *toolbars*/atalhos de pasta na barra (Windows 10) que não são
  detectados automaticamente — ajuste até liberar o espaço.

Aparência (fonte):

- `barraTarefas.tamanhoFonte` (pontos, padrão `9`, limitado a 6–24): tamanho da
  fonte do texto na barra.
- `barraTarefas.corFonte`: `"auto"` (padrão — preto em barra clara, branco em
  barra escura, conforme a cor real da barra) ou um hex `"#RRGGBB"` (ex.:
  `"#FFD700"`). Valores inválidos voltam para `"auto"`.
- `barraTarefas.formatoReset`: como o reset aparece no widget. `"restante"`
  (padrão) mostra o tempo regressivo (ex.: `33% (4:19h) | 68% (2d)`); `"exato"`
  mostra a hora/data do reset em horário local — só a hora se for hoje, ou com a
  data se for outro dia (ex.: `33% (19:20) | 68% (22/06, 19:59)`).
- Ambos são aplicados em ~1s ao editar o `config.json`, sem reiniciar. Evite uma
  `corFonte` igual à cor da barra (o fundo é transparente por *color-key*, então
  o texto sumiria).

Limitações:

- Funciona na barra padrão do Windows 11; barras modificadas
  (ExplorerPatcher/StartAllBack) podem se comportar de forma diferente.
- A mistura com barras translúcidas/acrílicas é aproximada (color-key), não um
  blend perfeito.

No Linux esse recurso não se aplica; o uso continua disponível no tooltip e no
título do tray.

## Widget da área de trabalho

Além da barra de tarefas, o app pode abrir um **widget flutuante na área de
trabalho**: uma janela sem moldura, opcionalmente sempre na frente, com um card
compacto por provedor (Claude/Codex). Os dados vêm do comando `get_widget_state`
(mesmo snapshot do tray/"Uso atual", sem rede). É ligado pela aba **Widget** das
Configurações (ou por `widget.habilitado` no `config.json`) e aplicado em ~1s,
sem reiniciar.

- **Conteúdo**: `mostraClaude`/`mostraCodex` escolhem quais provedores aparecem
  (além de o provedor estar `habilitado`) e, com duas contas no provedor, um
  interruptor por conta na aba Widget escolhe quais delas aparecem; `janelas` e
  `formatoReset` funcionam igual aos da barra (sessão/semanal e tempo restante vs.
  hora/data exata), com a opção extra `"nenhum"` no `formatoReset` que oculta o reset.
- **Modo de exibição** (`modo`): como cada provedor aparece — `"completo"`
  (padrão; cards com barras), `"minimo"` (uma linha por conta: o ícone do provedor e
  as porcentagens) ou `"anelduplo"` (anéis de progresso concêntricos, sessão no
  anel externo e semanal no interno; com uma janela só, desenha apenas o anel
  interno e encolhe).
- **Duas contas**: no mínimo, as linhas se agrupam em **Principal** e **Secundário**.
  No completo e no anel duplo, o card mostra uma conta por vez (a principal ao abrir):
  com o mouse sobre ele aparece o botão **⇄**, que alterna a conta com animação, e
  abaixo do card ficam os pontinhos e o apelido da conta na tela. O resto do card
  continua servindo para arrastar o widget. Uma conta com erro mantém a altura do
  card normal.
- **Posição e tamanho**: arraste a janela para reposicionar e redimensione pelas
  bordas; a posição e o tamanho são lembrados entre execuções. Na primeira vez o
  widget ajusta a altura ao conteúdo.
- **Sempre na frente**: `sempreNaFrente` mantém o widget acima das demais janelas.
- **Fundo**: `fundo` aceita uma imagem ou GIF (PNG, JPG, GIF, WEBP ou BMP) aplicada
  como plano de fundo do painel; lido sob demanda via `read_widget_background`.
  Vazio = sem fundo (só o painel escuro). A `opacidade` (0–100) controla o quanto
  o fundo aparece atrás dos cards.
- Os cantos arredondados são feitos pelo DWM na própria janela (sem
  transparência do WebView), evitando serrilhado nas curvas.

Em macOS o widget é ignorado.

## Dashboards no navegador (servidor HTTP com PIN)

Opcionalmente, o app pode servir os **dashboards de uso pelo navegador**, além da
janela nativa. É um servidor HTTP embutido no próprio app (não precisa de processo
externo), ligado pela aba **Servidor** das Configurações (ou por `servidor` no
`config.json`).

- **Mesma interface**: o servidor entrega a **mesma SPA** da janela nativa; o
  frontend detecta que está no navegador e troca o IPC do Tauri por `fetch`
  (`src/ipc.ts`).
- **Somente leitura**: expõe apenas **Uso atual**, **Dashboard Claude** e
  **Dashboard Codex**. As telas de **Envio** e **Configurações** e as credenciais
  (cookie do Claude, token do Codex) **não** ficam acessíveis — o backend recusa
  qualquer comando fora dessa lista.
- **Acesso por PIN**: ao abrir, o navegador pede um **PIN** (tela de login →
  cookie de sessão em memória). Sem PIN definido, o servidor não inicia mesmo
  habilitado.
- **Bind configurável**: `host` `127.0.0.1` (padrão) aceita só conexões locais;
  `0.0.0.0` aceita da rede. O servidor fala **HTTP puro** — para expor com
  **HTTPS**, coloque um proxy/túnel na frente (ex.: Cloudflare Tunnel) que termine
  o TLS.
- **Aplicação imediata**: ligar/desligar e trocar `host`/`porta`/`pin` nas
  Configurações **(re)inicia** o servidor na hora (a troca de host/porta encerra as
  sessões abertas). O PIN é relido a cada login.

> Observação: o servidor entrega os assets **embutidos no app empacotado**. Em
> `npm run tauri dev` os assets não ficam embutidos, então a API responde mas a
> página pode não ser servida — teste o recurso no app compilado (ou atrás do
> proxy).

## Rodando localmente

Pré-requisitos:

- Node.js
- Rust
- Dependências do Tauri v2 para sua plataforma

Comandos:

```bash
npm install
npm run tauri dev
```

Build local:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

## Instaladores

O repositório já está preparado para gerar:

- Windows: `.msi` (instalador)
- Linux: `AppImage`

Arquivos relevantes:

- Workflow único de build/release: [.github/workflows/release.yml](.github/workflows/release.yml)
- Config Windows: [src-tauri/tauri.windows.conf.json](src-tauri/tauri.windows.conf.json)
- Config Linux: [src-tauri/tauri.linux.conf.json](src-tauri/tauri.linux.conf.json)

O workflow de release roda:

- automaticamente em `push` para `main`, **apenas no repositório oficial
  (upstream)** — pushes em forks não publicam release
- a cada execução define a versão como `0.2.<número da execução>` (o updater
  compara semver, então a versão precisa incrementar a cada build)
- assina os artefatos e gera o `latest.json` (manifesto consumido pelo updater)
- sempre recria a release `main-latest` com os artefatos mais recentes

Para publicar no GitHub Releases, garanta que o repositório permita `GITHUB_TOKEN` com permissão de escrita em Actions.

## Atualizações automáticas (OTA)

O app se atualiza sozinho usando o updater oficial do Tauri v2. Ao iniciar, ele
verifica em segundo plano se há uma versão mais nova publicada na release
`main-latest` do repositório oficial; havendo, abre uma **janela de novidades**
que mostra o changelog antes de baixar — o **"delta"**: as novidades de **todas**
as versões entre a instalada e a mais nova, não só a da versão alvo. O usuário
**confirma** a instalação (com barra de progresso) e, ao concluir, o app reinicia
já na versão nova. Também há o item **Buscar atualizações** no menu do tray para
checar na hora.

O changelog exibido vem do `CHANGELOG.md` do projeto (buscado em runtime), e não
do campo `notes` do `latest.json` — esse campo agora vai vazio. A mesma fonte
alimenta as **Novidades** (na tela **Sobre**).

- Cobre as versões **instaladas**: Windows (`.msi`) e Linux (`AppImage`). A
  verificação compara a versão instalada com a do manifesto.
- A assinatura é obrigatória: cada release é assinada com a chave privada do
  projeto (secret no repositório) e o app só aceita pacotes cuja assinatura bata
  com a chave pública embutida (`plugins.updater.pubkey` no `tauri.conf.json`).
- Quem já tinha uma versão **sem** o updater precisa instalar manualmente uma vez
  para passar a receber as atualizações automáticas dali em diante.

Para publicar updates, o repositório oficial precisa do secret de assinatura
`TAURI_SIGNING_PRIVATE_KEY` configurado em Actions (a chave atual foi gerada sem
senha, então o secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, referenciado pelo
workflow, pode ficar vazio). Guarde a chave privada com segurança: sem ela não é
possível assinar updates que os apps já instalados aceitem.

## Dashboard Grafana

Exemplo sanitizado:

- [docs/grafana-dashboard.example.json](docs/grafana-dashboard.example.json)

Esse arquivo mantém:

- gráfico por usuário/ferramenta
- gauges de Codex e Claude
- tabela com últimos logs
- variáveis de filtro por `usuario` e `ferramenta`

## Limitações conhecidas

Linux:

- O suporte a tray depende do ambiente gráfico
- GNOME pode exigir suporte adicional a AppIndicator/StatusNotifierItem
- Tooltip de tray pode variar entre distribuições

Claude:

- A autenticação é pelo **login pelo navegador**, que captura a sessão e o
  Organization ID e os salva no app. Como o uso é medido **por organização**, se a conta tiver
  mais de uma a janela de login pede para escolher qual usar (mostrando o uso de cada uma) em vez
  de adivinhar — evita coletar da organização errada e reportar 0%
- A sessão web **expira** e **não há renovação automática** (não existe refresh
  token): quando expira, a coleta recebe 401/403 e o app pede para **reconectar**
- Uma conta no plano gratuito não tem janelas de limite na API: aparece como "Sem
  limites de uso para acompanhar" e não é enviada ao Loki
- A **reabertura automática de sessão** depende do Claude Code CLI instalado e
  logado com a assinatura; sem ele a opção fica sem efeito e a falha aparece na
  aba Claude das Configurações
- O disparo roda com o **raciocínio no mínimo** e **sem servidores MCP**, para
  custar o mínimo possível da cota: ele só precisa existir para abrir a janela.
  O **modelo não é fixado** — vale o que estiver no seu `~/.claude/settings.json`,
  e nada ali é alterado (o `--settings` do CLI só vale para aquela execução)

Codex:

- A autenticação é pelo **login pelo navegador** (OAuth), que salva e renova os
  tokens no próprio app
- Para conectar uma **segunda conta**, o login pede a tela de entrada da OpenAI; se o
  navegador ainda assim entrar direto na conta já conectada, saia dela no navegador
  (ou use uma janela anônima) e tente de novo
- O login pelo navegador abre a porta local `1455` para receber o retorno do OAuth
  (mesma porta usada pelo Codex CLI); deixe-a livre durante a conexão
- A **abertura automática de sessão** depende do Codex CLI instalado (o login dele não
  é usado); sem ele, a falha aparece na conta, na aba Codex das Configurações
- Uma versão anterior do app que grave o `contas.json` (ex.: ao mudar um apelido)
  descarta os ajustes da abertura automática, que ela não conhece: o recurso volta
  desligado, que é o padrão seguro

## Estrutura do projeto

```text
index.html            # janela principal do app (menu lateral + secoes)
widget.html           # janela do widget flutuante da area de trabalho
update.html           # janela de novidades do aviso de atualizacao (OTA)
claude-org.html       # modal de escolha da org do Claude, aberto na janela de login
CHANGELOG.md          # changelog (fonte das novidades exibidas no app)
src/
  main.ts             # shell: navegacao entre Envio de dados, Uso atual, Dashboards, Configuracoes e Sobre
  ipc.ts              # camada de IPC unificada: invoke nativo (Tauri) ou fetch /api/invoke (navegador)
  anima.ts            # animacoes compartilhadas de troca de tela/aba (altura do painel + fade do conteudo)
  tabs.ts             # pilula deslizante das barras de abas (um fundo por barra, que anda ate a aba clicada)
  envio.ts            # tela "Envio de dados" (pausa/envio por provedor, historico)
  usage.ts            # tela "Uso atual" (consome get_usage; rebusca sozinha, sem botão manual)
  usage-sim.ts        # so' no `tauri dev`: cenarios simulados de contas para avaliar o layout do "Uso atual"
  usage-format.ts     # helpers de formatacao/icones compartilhados (uso, reset, cores)
  dashboard.ts        # dashboard de uso do Claude Code (consome get_stats)
  codex-dashboard.ts  # dashboard de uso do Codex (consome get_codex_stats)
  chart-utils.ts      # helpers compartilhados pelos dois dashboards (rotulos de data, tooltip, periodo)
  sobre.ts            # tela "Sobre" (versao, status de atualizacao via update-status, novidades)
  update-status.ts    # verifica atualizacao ao abrir o app e liga o selo no item "Sobre" do menu
  novidades.ts        # historico de versoes (Novidades), renderizado na tela Sobre (get_changelog)
  changelog.ts        # parser + renderer de markdown do changelog (compartilhado)
  update.ts           # janela de novidades do OTA (delta de versoes; get_changelog/install_update)
  claude-org.ts       # escolha da org do Claude na janela de login (claude_login_orgs/claude_select_org)
  settings.ts         # configuracoes com abas e auto-save (consome get_settings/save_settings)
  widget.ts           # widget da area de trabalho (consome get_widget_state)
  widget-modos.ts     # modos de exibicao do widget ("minimo", "anelduplo") e pecas das varias contas
  vite-env.d.ts       # tipos do Vite (import.meta.env)
  styles.css

src-tauri/
  src/
    lib.rs             # tray, worker de coleta, janela do widget, janela do OTA e comandos IPC
                       # (get_stats/get_settings/save_settings/get_usage/force_collect/
                       #  get_widget_state/read_widget_background/pick_widget_background/
                       #  get_envio_state/set_envio_paused/set_envio_provider/clear_send_log/
                       #  check_updates_now/check_update_status/get_pending_update/install_update/
                       #  open_update_window/get_changelog/open_external/show_app_menu)
    main.rs
    usage_dashboard.rs # coleta as estatisticas do dashboard (Claude, arquivos locais)
    codex_dashboard.rs # historico diario de uso do Codex (API wham, get_codex_stats)
    contas.rs          # varias contas por provedor: arquivos (principal no legado, extras em contas/),
                       # preferencias em contas.json, principal/remocao e dedupe por identidade
    codex_auth.rs      # login do Codex pelo navegador (OAuth+PKCE), refresh dos tokens e
                       # copia do login usada pela sessao automatica
    sessao_auto_codex.rs # abertura automatica da janela de 5h do Codex (por conta, via Codex CLI)
    claude_auth.rs     # login do Claude pelo navegador (captura o sessionKey do webview) e org
    http_server.rs     # servidor HTTP opcional: serve os dashboards no navegador com PIN (somente leitura)
    taskbar_widget.rs  # widget da barra de tarefas (somente Windows)
  tauri.conf.json
  tauri.windows.conf.json
  tauri.linux.conf.json

docs/
  teste.http
  grafana-dashboard.example.json
```

## Próximos passos

- renovação e tratamento melhor de credenciais expiradas
- mais logs de sucesso no backend
- pacote Linux adicional como AppImage, se fizer sentido
