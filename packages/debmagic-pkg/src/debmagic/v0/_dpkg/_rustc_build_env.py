"""
Implement the build environment logic provided by /usr/share/rustc/architecture.mk
"""


def _rust_cpu(cpu: str, arch: str) -> str:
    # $(subst i586,i686,...)
    cpu = cpu.replace("i586", "i686")

    if "-riscv64-" in f"-{arch}-":
        return cpu.replace("riscv64", "riscv64gc")
    elif "-armhf-" in f"-{arch}-":
        return cpu.replace("arm", "armv7")
    elif "-armel-" in f"-{arch}-":
        return cpu.replace("arm", "armv5te")

    return cpu


def _rust_os(system: str, arch_os: str) -> str:
    if "-hurd-" in f"-{arch_os}-":
        return system.replace("gnu", "hurd-gnu")
    return system


def _get_rust_type(prefix: str, env_vars: dict[str, str]) -> str:
    cpu = env_vars.get(f"{prefix}_GNU_CPU", "")
    arch = env_vars.get(f"{prefix}_ARCH", "")
    system = env_vars.get(f"{prefix}_GNU_SYSTEM", "")
    arch_os = env_vars.get(f"{prefix}_ARCH_OS", "")

    r_cpu = _rust_cpu(cpu, arch)
    r_os = _rust_os(system, arch_os)

    return f"{r_cpu}-unknown-{r_os}"


def build_rustc_build_env(env: dict[str, str]):
    for machine in ["BUILD", "HOST", "TARGET"]:
        var_name = f"DEB_{machine}_RUST_TYPE"
        env[var_name] = _get_rust_type(f"DEB_{machine}", env)

    # Fallback for older dpkg versions (ifeq check)
    if env["DEB_TARGET_RUST_TYPE"] == "-unknown-":
        env["DEB_TARGET_RUST_TYPE"] = env["DEB_HOST_RUST_TYPE"]
