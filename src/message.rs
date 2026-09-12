use crate::http::AppResponse;
use crate::vars::ResolvedRequest;

#[derive(Debug)]
pub enum Message {
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    SendRequest,
    ResponseReceived(AppResponse),
    ResponseError(String),
    ToggleFocus,
    ScrollUp,
    ScrollDown,
    ScrollTop,
    ScrollBottom,
    ScrollStart,
    ScrollEnd,
    ScrollLeft,
    ScrollRight,
    ReloadFile,
    ToggleHelp,
    ToggleRequestDetail,
    Quit,
    Resize(u16, u16),
}

#[derive(Debug)]
pub enum Command {
    SendHttp(ResolvedRequest),
    Quit,
    None,
}
