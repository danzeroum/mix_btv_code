//! Gera os stubs tonic a partir de `schemas/proto/*.proto`.
//!
//! Usa o `protoc` vendorizado (`protoc-bin-vendored`) em vez do binário de
//! sistema — evita exigir uma instalação de protobuf no ambiente de build.
//! Reexecuta quando qualquer `.proto` muda (`cargo:rerun-if-changed`).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);

    let proto_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/proto");
    // llm.proto é folha; core.proto o importa; squad/promptforge são
    // autocontidos. tonic resolve os imports via o include path (proto_dir).
    let protos = [
        "llm.proto",
        "core.proto",
        "squad.proto",
        "promptforge.proto",
        "memory.proto",
    ];

    for proto in protos {
        println!("cargo:rerun-if-changed={proto_dir}/{proto}");
    }

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        // Fase 7 Onda 4: `SquadEvent` (e os tipos aninhados do `oneof payload`)
        // vai direto pro SSE do agente web — "sem DTO espelho", serde direto
        // no tipo gerado.
        .type_attribute(".forge.squad.v1", "#[derive(serde::Serialize)]")
        // Fase 7 Onda 5: mesma técnica para `GeneratorInfo`/`GeneratorField`,
        // que viajam como JSON em `GET /api/prompt/generators`.
        .type_attribute(".forge.promptforge.v1", "#[derive(serde::Serialize)]")
        // Fase 7 Onda 8: `MemorySummary`/`MemoryMatch` vão direto pro
        // `GET /api/memory`/`POST /api/memory/recall` — mesma técnica.
        .type_attribute(".forge.memory.v1", "#[derive(serde::Serialize)]")
        .compile_protos(&protos.map(|p| format!("{proto_dir}/{p}")), &[proto_dir])?;
    Ok(())
}
