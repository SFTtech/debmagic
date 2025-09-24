from pathlib import Path
from dataclasses import dataclass

import inspect


@dataclass
class BaseFrame:
    path: Path
    local_vars: dict


def get_base_frame() -> BaseFrame:
    for frame in inspect.stack():
        file_path = Path(frame.filename)
        # TODO further validation, be in debian/
        if file_path.name == "rules":
            return BaseFrame(
                path=file_path.parent.parent.resolve(),
                local_vars=frame.frame.f_locals,
            )
    raise RuntimeError("not called from 'rules' file")
