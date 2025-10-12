import inspect
from dataclasses import dataclass
from pathlib import Path


@dataclass
class RulesFile:
    package_dir: Path
    local_vars: dict


def find_rules_file() -> RulesFile:
    for frame in inspect.stack():
        file_path = Path(frame.filename)
        # TODO further validation, be in debian/
        if file_path.name == "rules" or file_path.name == "rules.py":
            return RulesFile(
                package_dir=file_path.parent.parent.resolve(),
                local_vars=frame.frame.f_locals,
            )
    raise RuntimeError("not called from 'rules' file")
