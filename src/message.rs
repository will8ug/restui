use crate::http::AppResponse;
use crate::vars::ResolvedRequest;

#[derive(Debug)]
pub enum Message {
    SelectNext,
    SelectPrev,
    SendRequest,
    ResponseReceived(AppResponse),
    ResponseError(String),
    ToggleFocus,
    ScrollUp,
    ScrollDown,
    ReloadFile,
    ToggleHelp,
    Quit,
    Resize(u16, u16),
}

#[derive(Debug)]
pub enum Command {
    SendHttp(ResolvedRequest),
    Quit,
    None,
}
