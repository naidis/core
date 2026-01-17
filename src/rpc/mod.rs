use jsonrpsee::RpcModule;

mod methods;

pub fn create_router() -> RpcModule<()> {
    let mut module = RpcModule::new(());

    methods::register_system_methods(&mut module);
    methods::register_youtube_methods(&mut module);
    methods::register_pdf_methods(&mut module);
    methods::register_ai_methods(&mut module);
    methods::register_rss_methods(&mut module);

    module
}
