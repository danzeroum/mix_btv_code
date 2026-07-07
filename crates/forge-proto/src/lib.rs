//! Stubs gRPC gerados de `schemas/proto/` (tonic).
//!
//! Fonte única do contrato: os `.proto` em `schemas/proto/`. Mudança
//! breaking = novo arquivo `.proto` (ex.: `promptforge_v2.proto`) + ADR.
//!
//! A estrutura de módulos espelha a hierarquia de pacotes protobuf
//! (`forge.core.v1` → `forge::core::v1`), porque o prost gera referências
//! entre pacotes por caminho relativo (`super::super::llm::v1::LlmRequest`
//! em `core` referenciando `llm`) — só funciona com o aninhamento correto.

pub mod forge {
    pub mod llm {
        pub mod v1 {
            tonic::include_proto!("forge.llm.v1");
        }
    }
    pub mod core {
        pub mod v1 {
            tonic::include_proto!("forge.core.v1");
        }
    }
    pub mod squad {
        pub mod v1 {
            tonic::include_proto!("forge.squad.v1");
        }
    }
    pub mod promptforge {
        pub mod v1 {
            tonic::include_proto!("forge.promptforge.v1");
        }
    }
    pub mod memory {
        pub mod v1 {
            tonic::include_proto!("forge.memory.v1");
        }
    }
}

// Aliases curtos e estáveis para os consumidores. `promptforge` já era
// exposto assim (forge-sidecar depende disso) — mantido; os demais seguem
// o mesmo formato.
pub mod promptforge {
    pub use crate::forge::promptforge::v1::*;
}
pub mod llm {
    pub use crate::forge::llm::v1::*;
}
pub mod core {
    pub use crate::forge::core::v1::*;
}
pub mod squad {
    pub use crate::forge::squad::v1::*;
}
pub mod memory {
    pub use crate::forge::memory::v1::*;
}
