use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

mod dirs;
#[cfg(windows)]
mod msvc;

pub fn setup_cargo_reruns() {
    fn emit(path: &Path) {
        println!("cargo::rerun-if-changed={}", path.display())
    }

    println!("cargo::rerun-if-changed=build/");
    println!("cargo::rerun-if-changed=m64prs-build-all.py");
    #[cfg(windows)]
    println!("cargo::rerun-if-changed=m64prs-vs-deps.sln");
    // mupen64plus-core-tas
    {
        let core_dir = Path::new(dirs::M64P_CORE_DIR);
        for entry in walkdir::WalkDir::new(&core_dir.join("src")) {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().is_file()
                && !path
                    .components()
                    .any(|comp| comp.as_os_str() == OsStr::new("asm_defines"))
            {
                emit(path);
            }
        }
        #[cfg(windows)]
        {
            let project_dir = &core_dir.join("projects/msvc");
            emit(&project_dir.join("mupen64plus-core.vcxproj"));
            emit(&project_dir.join("mupen64plus-core.vcxproj.filters"));
        }
        #[cfg(unix)]
        {
            let project_dir = &core_dir.join("projects/unix");
            emit(&project_dir.join("Makefile"));
        }
    }
}

#[cfg(windows)]
fn compile_m64p_deps(out_dir: &Path) {
    let (vs_env_arch, msbuild_platform) = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str()
    {
        "x86" => ("x86", "Win32"),
        "x86_64" => ("amd64", "x64"),
        _ => panic!("Target platform not supported!"),
    };
    let msbuild_config = match env::var("PROFILE").unwrap().as_str() {
        "debug" => "Debug",
        "release" => "Release",
        _ => unreachable!(),
    };

    let vs_env = msvc::vs_dev_env(vs_env_arch);

    let root_path = Path::new(dirs::ROOT_DIR);
    let sln_file = root_path.join("m64prs-vs-deps.sln");

    msvc::msbuild(
        &vs_env,
        &sln_file,
        out_dir,
        msbuild_config,
        msbuild_platform,
    );
}

#[cfg(unix)]
fn compile_m64p_deps(_out_dir: &Path) {
    use std::process::{Command, Stdio};

    let root_dir = PathBuf::from(dirs::ROOT_DIR);

    let cmd = Command::new("python3")
        .arg(root_dir.join("m64prs-build-all.py"))
        .arg("build")
        .stdout(os_pipe::dup_stderr().unwrap())
        .status()
        .expect("script invoke failed");

    assert!(cmd.success());
}

fn main() {
    // ./target contains all native outputs
    let out_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("target");
    fs::create_dir_all(&out_dir).unwrap();

    setup_cargo_reruns();
    compile_m64p_deps(&out_dir);
}
