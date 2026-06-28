//! Engine bin entrypoint。実体は [`bebeu_engine::entrypoint`] に集約。
fn main() -> anyhow::Result<()> {
    bebeu_engine::entrypoint()
}
