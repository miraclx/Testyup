//! Demonstrating how to create a custom Flow
//! here we open the browser for the user, making the use of InstalledAppFlow more convenient as
//! nothing has to be copy/pasted. Reason, the browser will open, the user accepts the requested
//! scope by clicking through e.g. the google oauth2, after this is done a local webserver started
//! by InstalledFlowAuthenticator will consume the token coming from the oauth2 server = no copy or
//! paste needed to continue with the operation.

use hyper;
use hyper_rustls;
use rand::{Rng, SeedableRng};
use rocket::{get, post, State};
use std::future::Future;
use std::pin::Pin;
use tokio::sync::{oneshot, RwLock};
use yup_oauth2::{authenticator::Authenticator, authenticator_delegate::InstalledFlowDelegate};

use super::ServerState;

pub async fn authenticate(
    state_id: String,
    auth_code_rx: oneshot::Receiver<String>,
) -> Authenticator<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>> {
    // Get an ApplicationSecret instance by some means. It contains the `client_id` and
    // `client_secret`, among other things.
    let secret = yup_oauth2::read_application_secret("credentials.json")
        .await
        .expect("client secret could not be read");

    // Instantiate the authenticator. It will choose a suitable authentication flow for you,
    // unless you replace  `None` with the desired Flow.
    // Provide your own `AuthenticatorDelegate` to adjust the way it operates and get feedback about
    // what's going on. You probably want to bring in your own `TokenStorage` to persist tokens and
    // retrieve them from storage.
    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::Interactive,
    )
    .persist_tokens_to_disk("tokencache.json")
    .flow_delegate(Box::new(InstalledFlowBrowserDelegate {
        state_id,
        auth_code_rx: RwLock::new(Some(auth_code_rx)),
    }))
    .build()
    .await
    .unwrap();

    return auth;
}

/// async function to be pinned by the `present_user_url` method of the trait
/// we use the existing `DefaultInstalledFlowDelegate::present_user_url` method as a fallback for
/// when the browser did not open for example, the user still see's the URL.
async fn browser_user_url(url: &str /* , need_code: bool */) /* -> Result<String, String>  */
{
    // Add client redirect here.
    if webbrowser::open(url).is_ok() {
        println!("webbrowser was successfully opened.");
    }
    // let def_delegate = DefaultInstalledFlowDelegate;
    // def_delegate.present_user_url(url, need_code).await
}

/// our custom delegate struct we will implement a flow delegate trait for:
/// in this case we will implement the `InstalledFlowDelegated` trait
pub struct InstalledFlowBrowserDelegate {
    pub state_id: String,
    pub auth_code_rx: RwLock<Option<oneshot::Receiver<String>>>,
}

/// here we implement only the present_user_url method with the added webbrowser opening
/// the other behaviour of the trait does not need to be changed.
impl InstalledFlowDelegate for InstalledFlowBrowserDelegate {
    /// the actual presenting of URL and browser opening happens in the function defined above here
    /// we only pin it
    fn present_user_url<'a>(
        &'a self,
        url: &'a str,
        _need_code: bool,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let url = format!("{}?state={}", url, self.state_id);
            browser_user_url(&url).await;
            if let Some(auth_code_rx) = self.auth_code_rx.write().await.take() {
                auth_code_rx
                    .await
                    .map_err(|_| "auth code receiver closed".to_string())
            } else {
                Err("auth code receiver already consumed".to_string())
            }
        })
    }

    fn redirect_uri(&self) -> Option<&str> {
        Some("http://localhost:8000/oauth2/client_login")
    }
}

#[post("/", data = "<name>")]
pub async fn create(server_state: &State<ServerState>, name: String) {
    let value = serde_json::json!({
        "function": "create_folder",
        "parameters": [
            name
        ],
    });
    let client = reqwest::Client::new();

    // todo! ensure this is non-blocking
    let mut rng = rand::rngs::StdRng::from_entropy();

    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789_-";

    let state_id: String = (0..256)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    let (tx, rx) = oneshot::channel();

    state
        .oauth_handlers
        .write()
        .await
        .insert(state_id.clone(), tx);

    let auth = authenticate(state_id, rx).await;
    let token = auth
        .token(&[
            "https://www.googleapis.com/auth/spreadsheets",
            "https://www.googleapis.com/auth/drive.readonly",
            "https://www.googleapis.com/auth/drive",
        ])
        .await
        .unwrap();
    let res = client.post("https://script.googleapis.com/v1/scripts/AKfycbzkB3j5U6pn_n9n2DN3OTLyjRA5owEN2C-u_sZyICYNCXwTs7DbTu0KIjTke2zQR5OE8g:run")
    .json(&value)
    .bearer_auth(token.as_str())
    .send()
    .await.unwrap();
    let val = res.text().await.unwrap();
    println!("{}", val)
}

// ? here's an example of a redirect uri
// ? https://google.zoom.us/google/oauth/client_login?
// ?  token=xxx
// ?  st=xxx
// ?  code=xxx
// ?  scope=email%20profile%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fuserinfo.profile%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fuserinfo.email%20openid
// ?  code_challenge=xxx
// ?  ver=5.13.0.13815
// ?  mode=token2
// ?  _x_zm_rtaid=xxx
// ?  _x_zm_rhtaid=xxx
// ?
// ? here, zoom is using token as a session id, and code as the auth code
// ? we are using auth_id as a session id, and code as the auth code
// todo! verify that state= will be proxied
// todo! verify that the redirect recipient is a get request
#[get("/client_login?<code>&<state>")]
pub async fn client_login(server_state: &State<ServerState>, code: String, state: String) {
    let state_id = state;
    let sender = match server_state.oauth_handlers.write().await.remove(&state_id) {
        Some(sender) => sender,
        None => {
            println!("No sender found for state_id {}", state_id);
            return;
        }
    };
    if let Err(_) = sender.send(code) {
        println!("Receiver dropped before sending code");
    }
}
