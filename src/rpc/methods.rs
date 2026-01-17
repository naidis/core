use jsonrpsee::RpcModule;

pub fn register_system_methods(module: &mut RpcModule<()>) {
    module
        .register_method("system.version", |_, _, _| "0.1.0")
        .unwrap();

    module
        .register_method("system.ping", |_, _, _| "pong")
        .unwrap();
}

pub fn register_youtube_methods(module: &mut RpcModule<()>) {
    module
        .register_async_method("youtube.extract_transcript", |params, _, _| async move {
            let url: String = params.one()?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!({
                "url": url,
                "transcript": [],
                "status": "not_implemented"
            }))
        })
        .unwrap();
}

pub fn register_pdf_methods(module: &mut RpcModule<()>) {
    module
        .register_async_method("pdf.extract_text", |params, _, _| async move {
            let path: String = params.one()?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!({
                "path": path,
                "text": "",
                "status": "not_implemented"
            }))
        })
        .unwrap();
}

pub fn register_ai_methods(module: &mut RpcModule<()>) {
    module
        .register_async_method("ai.chat", |params, _, _| async move {
            let message: String = params.one()?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!({
                "message": message,
                "response": "",
                "status": "not_implemented"
            }))
        })
        .unwrap();
}

pub fn register_rss_methods(module: &mut RpcModule<()>) {
    module
        .register_async_method("rss.parse_feed", |params, _, _| async move {
            let url: String = params.one()?;
            Ok::<_, jsonrpsee::types::ErrorObjectOwned>(serde_json::json!({
                "url": url,
                "items": [],
                "status": "not_implemented"
            }))
        })
        .unwrap();
}
