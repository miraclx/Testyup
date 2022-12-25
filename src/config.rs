//! Demonstrating how to create a custom Flow
//! here we open the browser for the user, making the use of InstalledAppFlow more convenient as
//! nothing has to be copy/pasted. Reason, the browser will open, the user accepts the requested
//! scope by clicking through e.g. the google oauth2, after this is done a local webserver started
//! by InstalledFlowAuthenticator will consume the token coming from the oauth2 server = no copy or
//! paste needed to continue with the operation.
use hyper;
use hyper_rustls;
use std::future::Future;
use std::pin::Pin;
use yup_oauth2::{
    authenticator::Authenticator,
    authenticator_delegate::{DefaultInstalledFlowDelegate, InstalledFlowDelegate},
};
use rocket::post;


pub async fn authenticate(
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
        yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk("tokencache.json")
    .flow_delegate(Box::new(InstalledFlowBrowserDelegate))
    .build()
    .await
    .unwrap();

    return auth;
}

/// async function to be pinned by the `present_user_url` method of the trait
/// we use the existing `DefaultInstalledFlowDelegate::present_user_url` method as a fallback for
/// when the browser did not open for example, the user still see's the URL.
async fn browser_user_url(url: &str, need_code: bool) -> Result<String, String> {
    // Add client redirect here.
    if webbrowser::open(url).is_ok() {
        println!("webbrowser was successfully opened.");
    }
    let def_delegate = DefaultInstalledFlowDelegate;
    def_delegate.present_user_url(url, need_code).await
}

/// our custom delegate struct we will implement a flow delegate trait for:
/// in this case we will implement the `InstalledFlowDelegated` trait
#[derive(Copy, Clone)]
pub struct InstalledFlowBrowserDelegate;

/// here we implement only the present_user_url method with the added webbrowser opening
/// the other behaviour of the trait does not need to be changed.
impl InstalledFlowDelegate for InstalledFlowBrowserDelegate {
    /// the actual presenting of URL and browser opening happens in the function defined above here
    /// we only pin it
    fn present_user_url<'a>(
        &'a self,
        url: &'a str,
        need_code: bool,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(browser_user_url(url, need_code))
    }

    fn redirect_uri(&self) -> Option<&str> {
        return Some("https://google.com");
    }
}

#[post("/", data = "<name>")]
pub async fn create(name: String)  {
    let value = serde_json::json!({
        "function": "create_folder",
        "parameters": [
            name
        ],
    });
    let client = reqwest::Client::new();
    let auth = authenticate().await;
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