from pathlib import Path

from zeta_forge.build_cli import Product, Project
from zeta_forge.config import load_repo_config
from zeta_forge.rust_engine import RUST_ACTIONS, RustEngine, RustTarget


def project(script_path: Path) -> Project:
    root = script_path.resolve().parent
    name = "zeta-practice"
    engine = RustEngine(
        root,
        load_repo_config(script_path),
        {name: RustTarget(name, root / "app", "fullstack", ("web",))},
        root / "target",
    )
    return Project(
        "zeta_practice",
        (Product(name, "rust", "Web/fullstack application", RUST_ACTIONS),),
        {"rust": engine},
        (name,),
        run_default=name,
    )
