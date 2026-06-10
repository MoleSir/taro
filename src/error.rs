#[thiserrorctx::context_error]
pub enum TaroError {
    #[error("Invalid args")]
    Args,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}