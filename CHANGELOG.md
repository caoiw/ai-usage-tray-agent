# Changelog

Todas as alterações relevantes deste projeto são documentadas aqui.
O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/).

As releases são geradas automaticamente a cada push no `main`, com versão
`0.2.<run_number>` (o número da execução do CI). O próprio **app** lê este
`CHANGELOG.md` (do `main`) para exibir as novidades: a **janela de atualização**
(OTA) mostra o *delta* — as novidades de todas as versões entre a instalada e a
mais nova — e a tela **Novidades** mostra o histórico completo. O campo `notes` do
`latest.json` não é mais usado para isso (vai vazio).

> **Como manter (cumulativo):**
> - Acumule as mudanças da próxima versão em **[Não lançado]** — só o que é
>   visível ou perceptível pelo usuário, em linguagem padrão da indústria.
>   Preencha **antes** de fazer merge no `main`.
> - O histórico é **cumulativo**: nunca apague seções de versões já lançadas.
> - A cada novo ciclo, **antes** de registrar novas mudanças, promova a
>   `[Não lançado]` anterior para uma seção da versão que foi publicada
>   (`## [0.2.<run>] - AAAA-MM-DD`) e recrie uma `[Não lançado]` vazia no topo.
>   A versão publicada é `0.2.<run_number>` da última **run de Release do upstream**;
>   pegue número e data com `gh run list --repo wzuqui/ai-usage-tray-agent
>   --workflow=release.yml -L 1 --json number,displayTitle,createdAt` (⚠️ a saída em
>   tabela do `gh run list` mostra o run **ID**, não o `run_number` — use `--json number`).
>   Passo a passo no `ONBOARDING.md`, seção "CHANGELOG.md — é runtime".
> - Escreva cada item em **uma única linha** (sem quebra manual): o aviso de
>   atualização é renderizado pelo app **já instalado** do usuário, e renderers
>   antigos podem exibir itens multi-linha de forma quebrada.

## [Não lançado]

### Adicionado
- Codex: opção “Reabrir a sessão automaticamente” — o app envia um “Oi” para abrir a janela de 5h assim que ela fica livre (ou em horários fixos), conta por conta, com o botão “Enviar teste”. Precisa do Codex CLI instalado.

## [0.2.72] - 2026-09-28

### Adicionado
- Várias contas: conecte até duas contas do Claude e duas do Codex (ex.: trabalho e pessoal), cada uma com seu apelido.
- Uso atual: com duas contas, o card do provedor mostra uma completa e a outra resumida ao lado; as setas do cabeçalho alternam entre elas.
- Widget: com duas contas, mostra as duas — no modo mínimo agrupadas em Principal e Secundário; nos outros modos, um botão alterna entre elas.
- Configurações: escolha quais contas aparecem no widget, qual a barra de tarefas mostra e qual é enviada ao Loki (por padrão, a principal).
- Dashboard Codex: seletor de conta no cabeçalho.

### Alterado
- Uso atual: um card por provedor, com sessão e semanal no mesmo container.
- Claude: a reabertura automática da sessão passa a acompanhar a conta em que o CLI está logado.
- Claude: a conta do plano gratuito aparece como "Sem limites de uso para acompanhar", em vez de erro.

### Corrigido
- Claude: o login com Google na janela de login do app passou a funcionar.

## [0.2.71] - 2026-09-26

### Removido
- Claude: removida a autenticação por campos manuais (Organization ID + Cookie) — quem usava esse modo precisa conectar a conta pelo navegador nas Configurações.
- Codex: removida a autenticação pelo arquivo auth.json — quem usava esse modo precisa conectar a conta pelo navegador nas Configurações.

### Corrigido
- Claude: fechar a janela de login sem entrar não mostra mais um erro técnico ("task panicked… RecvError") — o login só é cancelado.

### Alterado
- Claude: quando a conta tem mais de uma organização, a escolha aparece num modal na própria janela de login, em vez de um seletor na aba Claude das Configurações.
- Uso atual: quando um provedor informa só uma das janelas (ex.: o Codex só com o semanal), o bloco vazio some e a janela com dado ocupa o card inteiro.
- Uso atual: os blocos passaram a se chamar só "Sessão" e "Semanal", sem o "(5h)" e o "(7d)".

## [0.2.70] - 2026-09-15

### Corrigido
- Claude: a reabertura automática da sessão voltou a funcionar no Windows — desde a versão anterior todo disparo falhava com "Settings file not found" e a janela de 5h nunca chegava a ser aberta.

## [0.2.55] - 2026-09-14

### Adicionado
- Uso atual: na janela semanal, o reset que cai hoje ou amanhã aparece como "hoje, 17:00" e "amanhã, 17:00", em vez da data por extenso.
- Dashboards: o fundo da seleção também desliza entre os botões de período (30d/7d/Personalizado), como já acontecia nas abas.

### Alterado
- Sobre: a lista de Novidades passou a ocupar o resto da altura da janela, em vez de parar numa faixa fixa e deixar um vão embaixo — cabem bem mais versões de uma vez.
- Uso atual: o tempo até o reset e o horário/data ficam lado a lado, separados por um ponto, e o que não couber é cortado com reticências.
- Uso atual: ligar e desligar o gráfico ficou animado — os mini gráficos crescem e encolhem no lugar, em vez de os cards saltarem de tamanho.
- Dashboard Codex: ao trocar de aba, as barras do gráfico se transformam até os valores da aba escolhida, em vez de o gráfico inteiro ser redesenhado de uma vez.
- Dashboards: trocar o período anima a altura do painel, como já acontecia ao trocar de aba.
- Dashboards: clicar no período ou na aba que já está selecionada não refaz mais nada (antes o Dashboard Codex ia buscar os mesmos dados de novo).
- Claude: a reabertura automática da sessão passou a disparar com o raciocínio no mínimo e sem servidores MCP, consumindo bem menos da cota da janela de 5h; o modelo continua sendo o que você configurou no Claude Code, e nada nas suas configurações é alterado.

### Corrigido
- As animações de troca de aba e de tela ficaram estáveis: trocar de aba rápido não deixa mais o painel preso no tamanho da aba anterior, cortando o conteúdo novo; entrar nas abas Ferramentas e Projetos da Dashboard Claude passou a animar a altura como a saída já fazia; e voltar para uma tela pelo menu lateral não repete mais a animação da última troca de aba nem reabre do zero os blocos já abertos das Configurações.
- Dashboard Codex: trocar de período logo depois de abrir a tela, ou logo depois de voltar para o app, podia não ter efeito — o botão mudava e o conteúdo continuava sendo o do período anterior.
- Configurações: ao trocar de aba, o painel encolhia alguns pixels no começo da animação e os recuperava num salto no fim.
- Dashboard Codex: o conteúdo piscava ao alternar entre as abas Origens e Modelos.
- Clicar numa aba que já está aberta não faz mais o conteúdo dela piscar.
- Uso atual: ao desligar o gráfico, o interruptor voltava sozinho para ligado enquanto o aviso de confirmação ainda estava na tela.

## [0.2.54] - 2026-09-11

### Adicionado
- Configurações: ao mudar qualquer opção, aparece um "Configuração salva" no canto oposto ao título da tela, confirmando que a alteração foi gravada — antes o salvamento era automático e silencioso, sem nenhum retorno visual.
- A troca de telas e de abas agora é animada (o conteúdo aparece com um fade e o painel acompanha a altura da aba escolhida, em vez de saltar), e as abas ganharam realce ao passar o mouse, com o fundo da aba selecionada deslizando até a aba clicada.
- Configurações: os blocos que aparecem e somem conforme as opções (modo de autenticação, reabertura automática da sessão, avisos) passaram a crescer e encolher animados, em vez de o formulário saltar de tamanho.
- Configurações (aba Widget): o modo de exibição destaca o cartão sob o mouse e a troca de seleção acontece com transição de cor.
- Quem usa o Windows com os efeitos de animação desligados continua vendo o app sem nenhuma animação.

### Alterado
- Dashboard Claude (aba Ferramentas): nomes muito longos (as ferramentas MCP) não quebram mais o alinhamento da lista — agora cada nome ocupa uma linha só e é cortado no meio, preservando o começo e o fim; o nome completo fica no tooltip e alargar a janela revela mais caracteres.
- Todas as telas: a área de conteúdo ficou mais larga (até 1280px), aproveitando melhor as janelas grandes.
- Todas as telas: sobra menos espaço vazio no rodapé — antes a página podia ganhar barra de rolagem por causa da própria margem, mesmo com o conteúdo praticamente cabendo na janela.
- Configurações (aba Claude): a mensagem da última reabertura automática da sessão passou para dentro do quadro "Reabrir a sessão automaticamente", em vez de ficar solta abaixo dele.

## [0.2.53] - 2026-08-19

### Adicionado
- Claude: a reabertura automática da sessão agora tem dois modos — "Assim que expira" (o comportamento atual) e "Em horários fixos", em que você escolhe os horários do dia em que a sessão (5h) deve ser aberta.

### Corrigido
- Claude: quando a reabertura automática da sessão falha, o app passa a mostrar o motivo dado pelo Claude Code CLI (ex.: "Not logged in · Please run /login") na aba Claude e no log, em vez de apenas "terminou com exit code: 1".

## [0.2.52] - 2026-08-06

### Adicionado
- Claude: opção "Reabrir a sessão automaticamente" (aba Claude das Configurações) — quando a janela de sessão (5h) expira, o app envia um "Oi" sozinho para já iniciar a próxima, sem você precisar lembrar; vem desligada e usa o Claude Code CLI com o login da sua assinatura.
- Claude: a reabertura automática apaga o próprio rastro — o "Oi" não fica no histórico de conversas do Claude Code nem aparece como projeto na Dashboard Claude.

## [0.2.51] - 2026-07-13

### Alterado
- Codex: a OpenAI suspendeu temporariamente o limite de sessão (5h), deixando apenas o semanal (7d) — enquanto durar, a janela "Sessão (5h)" fica sem dados e o acompanhamento segue pelo semanal.

### Corrigido
- Codex conectado pelo navegador: o uso semanal (7d) voltou a aparecer corretamente, em vez de ficar em branco.

## [0.2.49] - 2026-07-09

### Adicionado
- Tela Uso atual: cada janela (sessão 5h e semanal 7d) ganhou um mini gráfico de linha com a evolução da porcentagem de uso nas últimas 5 horas, com detalhe de horário e valor ao passar o mouse.
- Tela Uso atual: opção "Gráfico" para mostrar ou ocultar os gráficos; ao desabilitar, o histórico (mantido só em memória) é descartado, com aviso de perda de dados e a opção "Não perguntar novamente".
- Tela Uso atual: botão "Reordenar" para ordenar os provedores arrastando os cards; a mesma ordem passa a valer também no widget e na barra de tarefas.
- A janela do app agora lembra o tamanho e a posição entre aberturas.

### Alterado
- A janela do app abre direto no tamanho e posição salvos e com fundo escuro, sem o "flash" branco nem o salto de tamanho ao abrir.

### Corrigido
- Dashboard Claude (abas Ferramentas e Projetos): ao reduzir a altura da janela, agora só a lista rola por dentro, sem criar rolagem também na janela.

## [0.2.48] - 2026-07-03

### Alterado
- Ícone da bandeja (Windows): o tooltip agora mostra só o nome do app; o uso continua no menu do tray e no widget da barra de tarefas.
- Configurações → Codex e Claude: a escolha do modo de autenticação virou botões lado a lado com ícone, no lugar da lista suspensa.
- Configurações → Servidor: o cabeçalho passou a ficar dentro do card e as opções logo abaixo, seguindo o mesmo layout das abas dos provedores.
- Configurações → Envio: os provedores foram para o topo, lado a lado e com uma divisória separando-os dos demais campos; o aviso de provedor sem credenciais ficou mais curto ("Sem credenciais").
- Configurações: as telas deixaram de mudar de largura quando a barra de rolagem vertical aparece.

## [0.2.47] - 2026-07-02

### Corrigido
- Configurações → Claude (login pelo navegador): quando a conta tem mais de uma organização, o app agora pede para você escolher qual usar (mostrando o uso atual de cada uma) em vez de escolher automaticamente a primeira — que podia ser uma organização sem uso, fazendo o app reportar 0% mesmo com a organização certa em uso.

## [0.2.46] - 2026-07-02

### Adicionado
- Configurações → Codex: novo modo de **login pelo navegador** para conectar a conta do Codex (OAuth), como alternativa ao caminho do `auth.json`; a sessão fica salva no app e é renovada automaticamente, com aviso para reconectar caso a renovação falhe.
- Configurações → Codex: no modo por arquivo, botão **Escolher…** para selecionar o `auth.json` pelo explorador de arquivos do sistema.
- Configurações → Claude: novo modo de **login pelo navegador** para conectar a conta, como alternativa ao preenchimento manual de Organization ID + Cookie; a conta é capturada automaticamente e o app avisa para reconectar quando a sessão expira.

## [0.2.44] - 2026-07-01

### Adicionado
- Configurações → Barra de tarefas e Widget: aviso, em cada provedor, quando ele está desativado ou sem credenciais (o interruptor continua operável).

### Alterado
- Configurações → Servidor: o liga/desliga ganhou destaque — virou um card com interruptor no cabeçalho, e os campos (endereço, porta, PIN) ficam esmaecidos quando o servidor está desligado.

## [0.2.43] - 2026-07-01

### Adicionado
- Configurações → Barra de tarefas e Widget: miniatura ilustrativa de como o widget aparece e onde fica na tela.
- Configurações → Widget: seletor do modo de exibição (Completo, Mínimo e Anel duplo) com uma prévia de cada modo — clicar na miniatura escolhe o modo.

### Alterado
- Configurações: nova aba **Envio** reúne o nome de exibição, a URL do Loki e o "Enviar ao Loki" de cada provedor (agora como interruptores, com aviso quando o provedor está desativado ou sem credenciais).
- Configurações → Codex e Claude: cada provedor virou um card com o logo e um interruptor para ativar/desativar a coleta; os campos de credenciais ficam esmaecidos quando o provedor está desativado.
- Configurações → Geral: a opção "Iniciar com o sistema" passou para esta aba (rótulo unificado em todos os sistemas operacionais).
- Configurações → Widget: o widget passa a aparecer quando ao menos um provedor está marcado (o interruptor separado "Mostrar widget na área de trabalho" foi removido).
- Configurações: os campos obrigatórios de cada provedor e o PIN do servidor agora são sinalizados como obrigatórios, com um aviso quando faltam para a coleta/servidor funcionar.

### Removido
- Configurações: aba **Sistema** — a opção "Iniciar com o sistema" foi para a aba Geral.

## [0.2.42] - 2026-06-30

### Alterado
- Envio de dados: a **contagem regressiva** do próximo envio passou para o **subtítulo da página** (como o "Atualizado há…" do Uso atual), e o histórico de envios ganhou um divisor abaixo do título.
- Janela de atualização: visual mais limpo — sem a seta ao lado do título e sem o sufixo "Atualização disponível" na barra de título da janela.
- Dashboard Claude e Dashboard Codex: divisor entre as abas/seletor de período e o conteúdo.

### Corrigido
- Abas: a aba ativa não perde mais o destaque ao alternar entre telas (Dashboard Claude, Dashboard Codex e Configurações).

### Removido
- Menu do tray (clique direito): removidos os itens de **status** (status geral e uso por provedor Codex/Claude) e o botão **Enviar agora** — o menu ficou só com as ações.

## [0.2.41] - 2026-06-30

### Corrigido
- Dashboard Claude: o painel não fica mais travado até reiniciar o app caso ocorra um erro ao carregar os dados de uso.

## [0.2.40] - 2026-06-30

### Adicionado
- Envio de dados: aviso quando **nenhum provedor está com "Enviar ao Loki" ativado** — o indicador muda para "Nenhum provedor enviando" e o histórico mostra um lembrete para ativar Claude ou Codex em Configurações.
- Envio de dados: o **histórico de envios** agora mostra os **dados enviados** ao Loki em cada envio com sucesso (uso da sessão 5h, uso semanal 7d e o reset), com o payload completo no tooltip.

### Alterado
- Uso atual: o indicador **"Atualizado há…"** saiu dos cards para o **subtítulo da página** (aparece uma única vez).

### Removido
- Uso atual: botão **Atualizar agora** (a tela já atualiza sozinha a cada poucos segundos).

### Corrigido
- Envio de dados: o histórico não pisca mais todas as linhas ao voltar de outra aba — só novas entradas que chegam com a tela aberta são destacadas.

## [0.2.39] - 2026-06-29

### Adicionado
- Configurações: opção **Enviar ao Loki** em cada provedor (Codex e Claude).
- Envio de dados: indicador "ao vivo" com **contagem regressiva** do próximo envio e o **status de envio de cada provedor**; o histórico **destaca** as novas entradas.

### Alterado
- Tela **Sobre**: primeira seção reorganizada (versão e status de atualização na mesma linha; repositório logo abaixo) e a lista de **Novidades** sem a caixa interna.
- Tela **Envio de dados**: visual simplificado.
- Widget: nome do provedor em cinza e o tempo de reset em branco, em todos os modos de exibição.

### Removido
- Envio de dados: a seção **Envio por provedor** (agora nas Configurações de cada provedor), o botão **Enviar agora** e a informação "Último envio com sucesso".

### Corrigido
- Tela Sobre: o **Copiar link** (menu do botão direito) do repositório agora copia o endereço correto.

## [0.2.37] - 2026-06-29

### Adicionado
- Configurações: nova aba **Servidor** para abrir os dashboards de uso no navegador, protegidos por um **PIN** (endereço, porta e PIN configuráveis; somente leitura — não expõe Configurações, Envio nem credenciais).

## [0.2.33] - 2026-06-29

### Adicionado
- Widget: novo seletor **Modo de exibição** com **Completo** (cards com barras, o atual), **Mínimo** (uma linha por provedor) e **Anel duplo** (anéis de progresso, sessão no anel externo e semanal no interno).
- Widget: opção **Nenhum** no formato do reset, que oculta o tempo/horário de reset em todos os modos.

### Alterado
- Widget: a janela pode ficar mais compacta (altura mínima menor), útil nos modos Mínimo e Anel duplo.
- Dashboard Claude: nas abas **Ferramentas** e **Projetos**, as barras de ranking ficaram com o mesmo comprimento e os nomes longos não são mais cortados.

### Corrigido
- Dashboard Codex: o indicador de carregamento não pisca mais ao reabrir a janela.

## [0.2.32] - 2026-06-25

### Corrigido
- O botão **Atualizar agora** na tela **Sobre** abria uma janela em branco; agora carrega as novidades normalmente.

## [0.2.31] - 2026-06-25

### Adicionado
- Selo **Atualização disponível** no item **Sobre** do menu quando há uma nova versão (verificada ao abrir o app).
- Dashboard Claude: novas visões **Ferramentas** (ferramentas mais usadas) e **Projetos** (uso por projeto).
- Dashboard Claude e Dashboard Codex: seletor de **intervalo de datas personalizado** (no Codex, até os últimos 90 dias).
- Dashboard Codex: indicador de carregamento e mensagem quando não há uso no período selecionado.

### Alterado
- Menu: **Uso atual** passou a ser o primeiro item, com **Envio de dados** logo abaixo.
- Dashboard Claude: abre nos **últimos 30 dias** por padrão e as cores do gráfico de modelos ganharam mais contraste.
- Telas mais limpas em **Uso atual**, **Dashboard Claude**, **Dashboard Codex** e **Configurações** (títulos e rodapés simplificados, sem subtítulos).

### Corrigido
- O ícone do Codex em **Uso atual** deixava de exibir o fundo ao trocar de tela.

## [0.2.30] - 2026-06-25

### Corrigido
- O changelog no aviso de atualização não quebra mais as linhas no meio das frases.

## [0.2.29] - 2026-06-25

### Adicionado
- Nova tela **Sobre** no menu: versão instalada, verificação de atualização (com **Atualizar agora** quando houver uma nova versão) e as **Novidades**.

### Alterado
- As **Novidades** deixaram de ser um item próprio do menu e agora ficam dentro da tela **Sobre** (em uma área de altura fixa com rolagem).

## [0.2.28] - 2026-06-25

### Adicionado
- Nova tela **Novidades**, com o histórico de versões do app.

### Alterado
- Ao atualizar pulando versões, o aviso de atualização agora mostra as novidades de **todas** as versões entre a sua e a mais nova, não só a da versão mais recente.

## [0.2.26] - 2026-06-24

### Adicionado
- O aviso de nova versão agora mostra as novidades da atualização em uma janela dedicada, com barra de progresso durante o download.

### Alterado
- As notas de cada versão passam a descrever as novidades de forma legível, em vez de um identificador técnico do build.

## Histórico

Versões anteriores à introdução deste arquivo (até a `0.2.25`) não possuem
changelog detalhado — eram builds automáticas do `main` identificadas apenas pelo
commit.
