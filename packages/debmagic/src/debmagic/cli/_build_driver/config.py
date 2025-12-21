from pydantic import BaseModel

from .driver_bare import BareDriverConfig
from .driver_docker import DockerDriverConfig
from .driver_lxd import LxdDriverConfig


class BuildDriverConfig(BaseModel):
    bare: BareDriverConfig = BareDriverConfig()
    docker: DockerDriverConfig = DockerDriverConfig()
    lxd: LxdDriverConfig = LxdDriverConfig()
