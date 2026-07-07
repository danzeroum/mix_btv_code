"""Regenera os stubs gRPC Python de schemas/proto/*.proto (grpcio-tools).

Saída: python/packages/forge-proto-py/src/forge_proto/ — nunca editar os
arquivos gerados à mão. Rode via `just gen-proto` (que também regenera o
lado Rust) ou `uv run --project python python ../scripts/gen_proto_py.py`.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROTO_DIR = ROOT / "schemas" / "proto"
OUT_DIR = ROOT / "python" / "packages" / "forge-proto-py" / "src" / "forge_proto"
PROTOS = ["llm.proto", "core.proto", "squad.proto", "promptforge.proto", "memory.proto"]

# grpc_tools gera import absoluto (`import x_pb2 as x__pb2`); como os stubs
# vivem dentro do pacote forge_proto, precisa virar import relativo. Isso
# aparece tanto nos `*_pb2_grpc.py` (import do próprio `*_pb2`) quanto nos
# `*_pb2.py` que importam outro proto (ex.: `core_pb2` importa `llm_pb2`,
# já que core.proto tem `import "llm.proto"`).
_ABS_IMPORT = re.compile(r"^import (\w+_pb2) as (\w+)$", re.MULTILINE)


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    cmd = [
        sys.executable,
        "-m",
        "grpc_tools.protoc",
        f"-I{PROTO_DIR}",
        f"--python_out={OUT_DIR}",
        f"--grpc_python_out={OUT_DIR}",
        f"--pyi_out={OUT_DIR}",
        *[str(PROTO_DIR / p) for p in PROTOS],
    ]
    subprocess.run(cmd, check=True)

    # Reescreve imports absolutos → relativos em todos os módulos gerados
    # (both *_pb2.py com cross-import e *_pb2_grpc.py).
    for py_file in OUT_DIR.glob("*_pb2*.py"):
        text = py_file.read_text(encoding="utf-8")
        patched = _ABS_IMPORT.sub(r"from . import \1 as \2", text)
        if patched != text:
            py_file.write_text(patched, encoding="utf-8")

    init_file = OUT_DIR / "__init__.py"
    if not init_file.exists():
        init_file.write_text(
            '"""Stubs gRPC gerados — nunca editar à mão. Regenere com scripts/gen_proto_py.py."""\n',
            encoding="utf-8",
        )

    generated = sorted(p.name for p in OUT_DIR.glob("*.py"))
    print(f"gerado(s) em {OUT_DIR}: {', '.join(generated)}")


if __name__ == "__main__":
    main()
