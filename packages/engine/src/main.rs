//! Engine bin entrypoint。実体は [`engine::entrypoint`] に集約。
fn main() -> anyhow::Result<()> {
    engine::entrypoint()
}
