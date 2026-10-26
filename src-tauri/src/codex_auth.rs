// Login do Codex pelo navegador (OAuth 2.0 com PKCE), alternativa ao caminho do
// `auth.json`. E' uma porta do fluxo do Codex CLI: sobe um servidor local em
// 127.0.0.1:1455 para receber o callback, abre o navegador do sistema na tela de
// login da OpenAI e troca o `authorization code` por tokens (access/refresh/id).
//
// Os tokens sao gravados em arquivos PROPRIOS do app, no MESMO formato do
// `~/.codex/auth.json` — assim os leitores que ja existem (coleta de uso em
// `lib.rs` e dashboard em `codex_dashboard.rs`) funcionam sem alteracao, bastando
// apontar para o arquivo. Cada conta tem o seu: a principal em `codex-auth.json` no
// `config_dir`, as extras em `contas/codex/` (ver `contas.rs`); por isso as funcoes
// recebem o caminho do arquivo da conta. NAO tocamos no `~/.codex/auth.json` do
// usuario.
//
// O `access_token` expira; guardamos `expires_at` e o renovamos via `refresh_token`
// em `ensure_fresh`, chamada antes de cada coleta no modo "navegador". O
// `account_id` (necessario ao dashboard) e o e-mail (so' para exibir "Conectado
// como ...") sao extraidos das claims do `id_token` (JWT).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tiny_http::{Header, Response, Server};

const ISSUER: &str = "https://auth.openai.com";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const PORT: u16 = 1455;
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const SCOPES: &str = "openid profile email offline_access";
/// Tempo maximo aguardando o usuario concluir o login no navegador.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Renova o `access_token` com esta antecedencia da expiracao (folga p/ relogio/rede).
const REFRESH_SKEW_SECS: i64 = 120;

/// Pagina simples exibida no navegador ao concluir o login com sucesso.
const SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html lang="pt-BR"><head><meta charset="UTF-8"><title>Autenticação concluída</title>
<style>body{font-family:system-ui,sans-serif;background:#0f1115;color:#e6e6e6;display:flex;
min-height:100vh;align-items:center;justify-content:center;margin:0}
.card{text-align:center;padding:2rem 2.5rem;background:#171a21;border-radius:14px;
box-shadow:0 10px 40px rgba(0,0,0,.4)}h1{font-size:1.25rem;margin:0 0 .5rem}
p{margin:0;color:#9aa4b2}</style></head>
<body><div class="card"><h1>Autenticação concluída ✓</h1>
<p>Você já pode fechar esta janela e voltar ao aplicativo.</p></div></body></html>"#;

/// Tokens persistidos no arquivo gerenciado. Espelha o formato do
/// `~/.codex/auth.json` (`tokens.*`, `last_refresh`) e acrescenta campos que so'
/// este app usa (`expires_at`, `email`); leitores externos ignoram os extras.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredAuth {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    #[serde(default)]
    tokens: StoredTokens,
    last_refresh: Option<String>,
    /// Instante de expiracao do `access_token`, em ms desde a epoch (UTC). Ausente
    /// = tratado como "renovar na proxima coleta".
    expires_at: Option<i64>,
    /// E-mail extraido do `id_token`, so' para exibir "Conectado como ...".
    email: Option<String>,
    /// Mensagem da ultima FALHA de renovacao automatica (refresh), se houve. Setada
    /// por `ensure_fresh` quando o refresh falha (ou nao ha refresh_token e o token
    /// expirou); limpa a cada login/refresh bem-sucedido. A UI usa isto para so'
    /// oferecer "Reconectar" quando a renovacao automatica esta' quebrada.
    refresh_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredTokens {
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    account_id: Option<String>,
}

/// Resposta do endpoint de token da OpenAI (login e refresh).
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
}

/// Claims relevantes do `id_token` (JWT). O `account_id` do ChatGPT vem dentro do
/// namespace `https://api.openai.com/auth`.
#[derive(Debug, Deserialize)]
struct IdClaims {
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/auth")]
    auth: Option<AuthClaim>,
}

#[derive(Debug, Deserialize)]
struct AuthClaim {
    chatgpt_account_id: Option<String>,
}

/// Arquivo legado de credenciais do Codex, que guarda a conta principal (e e' o
/// unico que versoes anteriores do app conhecem).
pub fn auth_file(config_dir: &Path) -> PathBuf {
    config_dir.join("codex-auth.json")
}

/// Flag de cancelamento do login em andamento (no maximo um por vez). O botao
/// "Cancelar" (via `cancel`) e o inicio de um novo login setam a flag do anterior,
/// fazendo o servidor de callback parar e liberar a porta.
fn login_cancel_flag() -> &'static Mutex<Option<Arc<AtomicBool>>> {
    static FLAG: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();
    FLAG.get_or_init(|| Mutex::new(None))
}

/// Sinaliza o cancelamento do login pelo navegador em andamento (se houver).
pub fn cancel() {
    let slot = login_cancel_flag()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(flag) = slot.as_ref() {
        flag.store(true, Ordering::SeqCst);
    }
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Gera `len` bytes aleatorios e os codifica em base64url (para o `state` e o
/// `code_verifier` do PKCE).
fn random_base64url(len: usize) -> Result<String, String> {
    let mut buf = vec![0u8; len];
    getrandom::fill(&mut buf).map_err(|error| format!("Falha ao gerar aleatoriedade: {error}"))?;
    Ok(base64url(&buf))
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        // NAO usar `cmd /C start` aqui: a URL do OAuth tem varios `&` (separadores
        // de query) e o `cmd` trata `&` como separador de comandos, quebrando a URL.
        // `rundll32 url.dll,FileProtocolHandler` abre no navegador padrao sem passar
        // pelo parser do cmd, preservando a URL inteira.
        let _ = Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = url; // outras plataformas: sem abertura automatica de navegador.
    }
}

/// Decodifica o segmento de payload do `id_token` (JWT) e le' as claims. Retorna
/// `None` se o token nao for um JWT valido/decodificavel.
fn decode_id_claims(id_token: &str) -> Option<IdClaims> {
    let payload = id_token.split('.').nth(1)?;
    // JWTs usam base64url sem padding, mas toleramos padding removendo os '='.
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Percent-decode simples (para valores da query do callback, ex.: `error_description`).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push((hi * 16 + lo) as u8);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        map.insert(percent_decode(key), percent_decode(value));
    }
    map
}

fn respond_text(request: tiny_http::Request, status: u16, text: &str) {
    let _ = request.respond(Response::from_string(text).with_status_code(status));
}

fn respond_html(request: tiny_http::Request, html: &str) {
    let mut response = Response::from_string(html).with_status_code(200);
    if let Ok(header) = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]) {
        response = response.with_header(header);
    }
    let _ = request.respond(response);
}

/// Abre o servidor de callback na porta 1455. Um login recem-cancelado pode ainda
/// estar liberando a porta, entao tentamos por um curto periodo antes de desistir.
fn bind_callback_server() -> Result<Server, String> {
    let mut last_error = String::new();
    for _ in 0..15 {
        match Server::http(("127.0.0.1", PORT)) {
            Ok(server) => return Ok(server),
            Err(error) => {
                last_error = error.to_string();
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    Err(format!(
        "Nao foi possivel abrir a porta {PORT} para o login (em uso por outro processo?): {last_error}"
    ))
}

/// Sobe o servidor local e aguarda (ate' `LOGIN_TIMEOUT`) o callback do OAuth,
/// validando o `state`. Devolve o `authorization code`. A espera e' fatiada para
/// checar o cancelamento (`cancel_flag`) periodicamente — assim o botao "Cancelar"
/// (ou um novo login) libera a porta sem esperar o timeout inteiro.
fn wait_for_code(state: &str, cancel_flag: &AtomicBool) -> Result<String, String> {
    let server = bind_callback_server()?;
    let deadline = Instant::now() + LOGIN_TIMEOUT;

    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err("Login cancelado.".to_string());
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| "Tempo limite aguardando o login no navegador.".to_string())?;

        // Espera em fatias curtas para re-checar cancelamento/deadline.
        let slice = remaining.min(Duration::from_millis(300));
        let request = match server.recv_timeout(slice) {
            Ok(Some(request)) => request,
            Ok(None) => continue,
            Err(error) => return Err(format!("Falha no servidor de callback: {error}")),
        };

        let raw_url = request.url().to_string();
        let (path, query) = raw_url.split_once('?').unwrap_or((raw_url.as_str(), ""));
        if path != "/auth/callback" {
            respond_text(request, 404, "Not found");
            continue;
        }

        let params = parse_query(query);
        if params.get("state").map(String::as_str) != Some(state) {
            respond_text(request, 400, "OAuth state invalido.");
            return Err("OAuth state invalido.".to_string());
        }
        if let Some(oauth_error) = params.get("error") {
            let description = params
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| oauth_error.clone());
            respond_text(
                request,
                400,
                &format!("Falha na autenticacao: {description}"),
            );
            return Err(format!("Erro OAuth: {description}"));
        }
        match params.get("code") {
            Some(code) if !code.is_empty() => {
                let code = code.clone();
                respond_html(request, SUCCESS_HTML);
                return Ok(code);
            }
            _ => {
                respond_text(request, 400, "Authorization code nao retornado.");
                return Err("Authorization code nao retornado.".to_string());
            }
        }
    }
}

/// Troca o `authorization code` por tokens (grant_type=authorization_code + PKCE).
fn exchange_code(client: &Client, code: &str, verifier: &str) -> Result<TokenResponse, String> {
    post_token(
        client,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ],
        "trocar o código por tokens",
    )
}

/// Renova os tokens usando o `refresh_token` (grant_type=refresh_token).
fn exchange_refresh(client: &Client, refresh_token: &str) -> Result<TokenResponse, String> {
    post_token(
        client,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
            ("scope", SCOPES),
        ],
        "renovar o token do Codex",
    )
}

fn post_token(client: &Client, form: &[(&str, &str)], acao: &str) -> Result<TokenResponse, String> {
    let response = client
        .post(format!("{ISSUER}/oauth/token"))
        .form(form)
        .send()
        .map_err(|error| format!("Falha HTTP ao {acao}: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "A OpenAI recusou a requisição ao {acao} (HTTP {status}): {body}"
        ));
    }
    response
        .json::<TokenResponse>()
        .map_err(|error| format!("Resposta de token inválida ao {acao}: {error}"))
}

/// Monta o `StoredAuth` a partir da resposta de token. Na renovacao, alguns campos
/// podem nao voltar (ex.: `refresh_token`, `id_token`); nesse caso reaproveita os
/// valores anteriores (`previous`).
fn stored_from_tokens(
    tokens: TokenResponse,
    previous: Option<&StoredAuth>,
) -> Result<StoredAuth, String> {
    let access_token = tokens
        .access_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "A OpenAI não retornou access_token.".to_string())?;

    let refresh_token = tokens
        .refresh_token
        .filter(|token| !token.is_empty())
        .or_else(|| previous.and_then(|prev| prev.tokens.refresh_token.clone()));

    let id_token = tokens
        .id_token
        .filter(|token| !token.is_empty())
        .or_else(|| previous.and_then(|prev| prev.tokens.id_token.clone()));

    let claims = id_token.as_deref().and_then(decode_id_claims);
    let account_id = claims
        .as_ref()
        .and_then(|claims| claims.auth.as_ref())
        .and_then(|auth| auth.chatgpt_account_id.clone())
        .filter(|value| !value.is_empty())
        .or_else(|| previous.and_then(|prev| prev.tokens.account_id.clone()));
    let email = claims
        .and_then(|claims| claims.email)
        .or_else(|| previous.and_then(|prev| prev.email.clone()));

    let expires_at = tokens.expires_in.map(|seconds| now_ms() + seconds * 1000);

    Ok(StoredAuth {
        openai_api_key: None,
        tokens: StoredTokens {
            id_token,
            access_token: Some(access_token),
            refresh_token,
            account_id,
        },
        last_refresh: Some(Utc::now().to_rfc3339()),
        expires_at,
        email,
        refresh_error: None,
    })
}

fn read_stored(path: &Path) -> Option<StoredAuth> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_stored(path: &Path, auth: &StoredAuth) -> Result<(), String> {
    let payload = serde_json::to_string_pretty(auth).map_err(|error| error.to_string())?;
    crate::contas::gravar_atomico(path, &format!("{payload}\n"))
}

/// Identidade de uma conta (base do id dela em `contas`): o workspace
/// (`account_id`) e o e-mail, os dois vindos das claims do `id_token`. Normalizada
/// para que a mesma conta, reconectada, caia na mesma chave.
fn identity_of(auth: &StoredAuth) -> String {
    format!(
        "{}|{}",
        auth.tokens.account_id.as_deref().unwrap_or("").trim(),
        auth.email.as_deref().unwrap_or("").trim().to_lowercase()
    )
}

/// Identidade e e-mail da conta gravada em `path`, ou `None` se nao ha' login valido.
pub fn identity(path: &Path) -> Option<(String, Option<String>)> {
    let auth = read_stored(path).filter(has_access_token)?;
    Some((identity_of(&auth), auth.email.clone()))
}

fn has_access_token(auth: &StoredAuth) -> bool {
    auth.tokens
        .access_token
        .as_deref()
        .map(|token| !token.is_empty())
        .unwrap_or(false)
}

/// Status do login pelo navegador, para a aba Codex das Configuracoes.
/// `needsReconnect` fica true quando ha' uma sessao salva mas a renovacao
/// automatica falhou (ou nao ha' como renovar e o token expirou) — so' nesse caso a
/// UI oferece "Reconectar".
fn status_value(auth: &StoredAuth) -> Value {
    let expired = auth.expires_at.map(|exp| now_ms() >= exp).unwrap_or(false);
    let no_refresh = auth
        .tokens
        .refresh_token
        .as_deref()
        .map(|token| token.is_empty())
        .unwrap_or(true);
    let needs_reconnect =
        has_access_token(auth) && (auth.refresh_error.is_some() || (expired && no_refresh));
    json!({
        "connected": has_access_token(auth),
        "needsReconnect": needs_reconnect,
        "email": auth.email,
        "expiresAt": auth.expires_at,
        "accountId": auth.tokens.account_id,
    })
}

// ---- API publica (consumida por lib.rs) ------------------------------------

/// Tokens de um login recem-concluido, ainda nao gravados: quem chama decide o
/// arquivo pela identidade da conta (ver `contas::destino_do_login`) e grava com
/// `save_login`.
pub struct CodexLogin(StoredAuth);

impl CodexLogin {
    pub fn identity(&self) -> String {
        identity_of(&self.0)
    }
}

/// Executa o login pelo navegador (BLOQUEANTE: sobe o servidor de callback, abre o
/// navegador e aguarda ate' `LOGIN_TIMEOUT`) e devolve os tokens, sem gravar. Deve
/// ser chamada fora da main thread (ex.: `spawn_blocking`).
///
/// `outra_conta`: ja' ha' conta do Codex conectada. O navegador do sistema costuma
/// ter a sessao da OpenAI aberta e devolveria a mesma conta sem perguntar; o
/// `prompt=login` pede a tela de login mesmo assim, para o usuario poder entrar
/// com outra.
pub fn login(client: &Client, outra_conta: bool) -> Result<CodexLogin, String> {
    // Cancela um login anterior que ainda esteja preso aguardando (libera a 1455) e
    // registra a flag deste login para que o botao "Cancelar" possa interrompe-lo.
    cancel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    *login_cancel_flag()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(cancel_flag.clone());

    let outcome = login_flow(client, outra_conta, &cancel_flag);

    // Limpa o slot se ainda for o nosso (nao pisa num login mais novo).
    {
        let mut slot = login_cancel_flag()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot
            .as_ref()
            .map(|flag| Arc::ptr_eq(flag, &cancel_flag))
            .unwrap_or(false)
        {
            *slot = None;
        }
    }
    outcome
}

fn login_flow(
    client: &Client,
    outra_conta: bool,
    cancel_flag: &AtomicBool,
) -> Result<CodexLogin, String> {
    let state = random_base64url(32)?;
    let verifier = random_base64url(64)?;
    let challenge = base64url(Sha256::digest(verifier.as_bytes()).as_slice());

    let mut params = vec![
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("scope", SCOPES),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "codex_cli_rs"),
        ("state", state.as_str()),
    ];
    if outra_conta {
        params.push(("prompt", "login"));
    }
    let auth_url = reqwest::Url::parse_with_params(&format!("{ISSUER}/oauth/authorize"), &params)
        .map_err(|error| format!("URL de autorização inválida: {error}"))?;

    open_browser(auth_url.as_str());

    let code = wait_for_code(&state, cancel_flag)?;
    let tokens = exchange_code(client, &code, &verifier)?;
    Ok(CodexLogin(stored_from_tokens(tokens, None)?))
}

/// Grava os tokens de um login em `path` e devolve o status
/// (`{connected,email,expiresAt,accountId}`).
pub fn save_login(path: &Path, login: &CodexLogin) -> Result<Value, String> {
    write_stored(path, &login.0)?;
    Ok(status_value(&login.0))
}

/// Status do login da conta gravada em `path` (sem rede).
pub fn status(path: &Path) -> Value {
    match read_stored(path) {
        Some(auth) => status_value(&auth),
        None => json!({ "connected": false, "email": Value::Null, "expiresAt": Value::Null }),
    }
}

/// Garante um `access_token` valido no arquivo da conta (`path`) e devolve o
/// caminho, para os leitores (coleta/dashboard) o lerem como um `auth.json` normal.
/// Renova via `refresh_token` quando o token esta' perto de expirar. Erro se nao ha
/// login.
pub fn ensure_fresh(client: &Client, path: &Path) -> Result<PathBuf, String> {
    // Uma renovacao por vez: o `refresh_token` e' de uso unico (a OpenAI o
    // rotaciona), entao duas renovacoes simultaneas da mesma conta (coleta,
    // dashboard, sessao automatica) fariam a segunda falhar com "refresh token
    // already used" e a conta pedir "Reconectar".
    let trava = trava_da_conta(path);
    let _trava = travar(&trava);
    let path = path.to_path_buf();
    let mut auth = ler_conectado(&path)?;

    let needs_refresh = match auth.expires_at {
        Some(expires_at) => now_ms() + REFRESH_SKEW_SECS * 1000 >= expires_at,
        None => true,
    };
    if needs_refresh {
        match auth
            .tokens
            .refresh_token
            .clone()
            .filter(|token| !token.is_empty())
        {
            Some(refresh_token) => match exchange_refresh(client, &refresh_token) {
                Ok(tokens) => {
                    // Sucesso limpa qualquer `refresh_error` anterior (via
                    // `stored_from_tokens`, que grava None).
                    auth = stored_from_tokens(tokens, Some(&auth))?;
                    write_stored(&path, &auth)?;
                }
                Err(error) => {
                    // Renovacao falhou: registra para a UI oferecer "Reconectar".
                    auth.refresh_error = Some(error.clone());
                    let _ = write_stored(&path, &auth);
                    return Err(error);
                }
            },
            None => {
                // Sem refresh_token: so' da' pra reconectar. Se o token ainda nao
                // expirou de fato, seguimos com ele; caso contrario, pedimos login.
                let expired = auth.expires_at.map(|exp| now_ms() >= exp).unwrap_or(false);
                if expired {
                    let error =
                        "Sessão do Codex expirada. Reconecte pelo login do navegador.".to_string();
                    auth.refresh_error = Some(error.clone());
                    let _ = write_stored(&path, &auth);
                    return Err(error);
                }
            }
        }
    }

    Ok(path)
}

/// Le' a conta de `path`, exigindo um login com `access_token`.
fn ler_conectado(path: &Path) -> Result<StoredAuth, String> {
    read_stored(path).filter(has_access_token).ok_or_else(|| {
        "Codex não conectado. Faça o login pelo navegador nas Configurações.".to_string()
    })
}

/// Trava da renovacao dos tokens da conta em `arquivo`. Uma por conta: a renovacao
/// segura a trava durante a chamada de rede, e uma conta lenta nao pode segurar as
/// outras.
fn trava_da_conta(arquivo: &Path) -> Arc<Mutex<()>> {
    static TRAVAS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    // O mesmo arquivo escrito de outro jeito (relativo, outra caixa no Windows) tem
    // de cair na mesma trava. Caminho absoluto e, no Windows, em minusculas — sem
    // `canonicalize`, que depende de o arquivo existir: durante a troca da conta
    // principal ele some por um instante, e a chave mudaria no meio do caminho.
    let absoluto = std::path::absolute(arquivo).unwrap_or_else(|_| arquivo.to_path_buf());
    let chave = if cfg!(windows) {
        PathBuf::from(absoluto.to_string_lossy().to_lowercase())
    } else {
        absoluto
    };
    TRAVAS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(chave)
        .or_default()
        .clone()
}

fn travar(trava: &Mutex<()>) -> std::sync::MutexGuard<'_, ()> {
    trava
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Grava em `destino` um `auth.json` no formato do Codex CLI com o login da conta
/// em `arquivo`, para o disparo da sessao automatica rodar **nessa** conta (com
/// `CODEX_HOME` apontando para a pasta de `destino`). Devolve a identidade da conta
/// copiada — da mesma leitura da copia —, para quem chama conferir que copiou a
/// conta que queria.
///
/// E' tambem a guarda de cobranca: so' copia login ChatGPT (assinatura). Uma conta
/// com chave de API gravada faria o CLI cobrar por requisicao, num pool que nem
/// abre a janela de 5h.
///
/// O `refresh_token` vai **vazio**: o CLI usa so' o `access_token` (quem chama roda
/// `ensure_fresh` antes, para ele estar valido) e nao tem como renovar o login. Uma
/// renovacao feita pelo CLI rotacionaria o `refresh_token` fora do arquivo do app,
/// e a coleta perderia o login. Vazio, e nao ausente/nulo: sem o campo, o CLI nao
/// reconhece o login (conferido no CLI 0.163). O `last_refresh` vai com o instante
/// atual, para o CLI nem tentar renovar.
pub fn copiar_para_cli(arquivo: &Path, destino: &Path) -> Result<String, String> {
    let auth = ler_conectado(arquivo)?;
    if auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty())
    {
        return Err(
            "a conta tem uma chave de API gravada; o disparo foi recusado para não gerar cobrança por uso da API."
                .to_string(),
        );
    }
    if auth.refresh_error.is_some() {
        return Err(
            "a sessão desta conta expirou. Reconecte a conta para a abertura automática funcionar."
                .to_string(),
        );
    }
    if auth.tokens.id_token.as_deref().is_none_or(str::is_empty) {
        return Err(
            "o login desta conta está incompleto (sem id_token). Reconecte a conta.".to_string(),
        );
    }
    let copia = json!({
        "OPENAI_API_KEY": Value::Null,
        "tokens": {
            "id_token": auth.tokens.id_token,
            "access_token": auth.tokens.access_token,
            "refresh_token": "",
            "account_id": auth.tokens.account_id,
        },
        "last_refresh": Utc::now().to_rfc3339(),
    });
    let conteudo = serde_json::to_string_pretty(&copia).map_err(|error| error.to_string())?;
    crate::contas::gravar_atomico(destino, &format!("{conteudo}\n"))?;
    Ok(identity_of(&auth))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ai-usage-codex-auth-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn conta() -> StoredAuth {
        StoredAuth {
            tokens: StoredTokens {
                id_token: Some("id".to_string()),
                access_token: Some("acesso".to_string()),
                refresh_token: Some("r1".to_string()),
                account_id: Some("conta-1".to_string()),
            },
            email: Some("teste@exemplo.com".to_string()),
            expires_at: Some(1),
            ..StoredAuth::default()
        }
    }

    #[test]
    fn copia_tem_o_formato_do_cli_sem_o_refresh_token_e_recusa_chave_de_api() {
        let dir = pasta("copia");
        let arquivo = dir.join("conta.json");
        let destino = dir.join("home").join("auth.json");
        write_stored(&arquivo, &conta()).unwrap();

        let identidade = copiar_para_cli(&arquivo, &destino).unwrap();
        assert_eq!(identidade, identity_of(&conta()));
        let gravado: Value =
            serde_json::from_str(&std::fs::read_to_string(&destino).unwrap()).unwrap();
        assert_eq!(gravado["tokens"]["access_token"], "acesso");
        // O CLI nao recebe como renovar o login (e rotacionar o token do app).
        assert_eq!(gravado["tokens"]["refresh_token"], "");
        assert!(gravado["OPENAI_API_KEY"].is_null());
        assert!(gravado["last_refresh"].is_string());
        // Campos so' do app nao vao para o CLI.
        assert!(gravado.get("email").is_none());
        // O arquivo da conta continua com o token de verdade.
        assert_eq!(
            read_stored(&arquivo)
                .unwrap()
                .tokens
                .refresh_token
                .as_deref(),
            Some("r1")
        );

        let mut com_chave = conta();
        com_chave.openai_api_key = Some("sk-teste".to_string());
        write_stored(&arquivo, &com_chave).unwrap();
        assert!(copiar_para_cli(&arquivo, &destino).is_err());

        let mut expirada = conta();
        expirada.refresh_error = Some("falhou".to_string());
        write_stored(&arquivo, &expirada).unwrap();
        assert!(copiar_para_cli(&arquivo, &destino).is_err());
    }
}
