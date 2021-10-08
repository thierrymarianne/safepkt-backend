use crate::domain::value_object::*;
use crate::infra::scaffold::{format_directory_path_to_scaffold, format_project_name};
use crate::infra::verification_runtime::docker::DockerContainerAPIClient;
use anyhow::Result;
use bollard::container::{Config, CreateContainerOptions};
use bollard::{models::*, Docker};
use color_eyre::Report;
use std::env;
use std::path;
use tracing::info;

pub static TARGET_RVT_DIRECTORY: &str = "/home/rust-verification-tools";
static TARGET_SOURCE_DIRECTORY: &str = "/ink/examples/source";

fn get_uid_gid() -> Result<String, Report> {
    let uid_gid = env::var("UID_GID")?;
    Ok(uid_gid)
}

fn get_rvt_directory() -> Result<String, Report> {
    let source_directory = env::var("RVT_DIRECTORY")?;
    Ok(source_directory)
}

fn get_rvt_container_image() -> Result<String, Report> {
    let container_image = env::var("RVT_DOCKER_IMAGE")?;
    Ok(container_image)
}

pub fn llvm_bitcode_generation_cmd_provider() -> StepProvider {
    |prefixed_hash: &str, bitcode: &str, _: Option<&str>| -> String {
        format!("cargo verify -v --bin {} -o {}", prefixed_hash, bitcode)
    }
}

pub fn symbolic_execution_cmd_provider() -> StepProvider {
    |_: &str, bitcode: &str, additional_flags: Option<&str>| -> String {
        match additional_flags {
            Some(flags) => {
                format!("klee --libc=klee {} {}", flags, bitcode)
            }
            None => format!(
                "klee --libc=klee --silent-klee-assume --warnings-only-to-file {}",
                bitcode
            ),
        }
    }
}

pub fn program_verification_cmd_provider() -> StepProvider {
    |prefixed_hash: &str, bitcode: &str, _: Option<&str>| -> String {
        format!(
            "cp /ink/examples/erc20/.abi /ink/examples/source && cargo verify --tests -v --bin {} -o {} && klee --libc=klee --silent-klee-assume --warnings-only-to-file {}",
            prefixed_hash, bitcode, bitcode
        )
    }
}

pub fn source_code_restoration_cmd_provider() -> StepProvider {
    |_: &str, _: &str, _: Option<&str>| -> String {
        // library or binary scaffolding
        // let path_to_source = [TARGET_SOURCE_DIRECTORY, "src", "main.rs"]
        let path_to_source = [TARGET_SOURCE_DIRECTORY, "src", "lib.rs"]
            .join(path::MAIN_SEPARATOR.to_string().as_str());

        format!("cat {}", path_to_source)
    }
}

fn get_configuration<'a>(
    command_parts: Vec<&'a str>,
    container_image: &'a str,
    project_id: &'a str,
    username: &'a str,
) -> Result<Config<&'a str>, Report> {
    let rvt_directory = get_rvt_directory()?;

    let host_config = HostConfig {
        auto_remove: Some(false),
        mounts: Some(vec![
            Mount {
                target: Some(TARGET_SOURCE_DIRECTORY.to_string()),
                source: Some(format_directory_path_to_scaffold(project_id)),
                typ: Some(MountTypeEnum::BIND),
                consistency: Some(String::from("default")),
                ..Default::default()
            },
            Mount {
                target: Some(TARGET_RVT_DIRECTORY.to_string()),
                source: Some(rvt_directory),
                typ: Some(MountTypeEnum::BIND),
                consistency: Some(String::from("default")),
                ..Default::default()
            },
        ]),
        network_mode: Some(String::from("host")),
        ..Default::default()
    };

    Ok(Config {
        cmd: Some(command_parts),
        host_config: Some(host_config),
        image: Some(container_image),
        user: Some(username),
        working_dir: Some(TARGET_SOURCE_DIRECTORY),
        ..Default::default()
    })
}

fn get_bitcode_filename(project_id: &str) -> String {
    format!("{}.bc", project_id)
}

pub async fn start_container(
    container_api_client: &DockerContainerAPIClient<Docker>,
    container_name: String,
    project_step: &StepInVerificationPlan<'_>,
) -> Result<(), Report> {
    let project_id = project_step.project_id().clone();
    let step = project_step.step;

    let container_image = get_rvt_container_image()?;
    let prefixed_hash = format_project_name(project_id.as_str());
    let prefixed_hash = prefixed_hash.as_str();

    let bitcode_file_name = get_bitcode_filename(project_id.as_str());
    let bitcode_file_name = bitcode_file_name.as_str();

    let command: String = step.step_provider()(prefixed_hash, bitcode_file_name, step.flags());
    let command = command.as_str();
    let command_parts = command.split(" ").collect::<Vec<&str>>();

    let maybe_uid_gid = get_uid_gid()?;

    let configuration = get_configuration(
        command_parts,
        container_image.as_str(),
        project_id.as_str(),
        maybe_uid_gid.as_str(),
    )?;

    info!(
        "About to start container with name {} based on image {}",
        container_name.as_str(),
        container_name
    );

    let id = container_api_client
        .client()
        .create_container(
            Some(CreateContainerOptions {
                name: container_name.as_str(),
            }),
            configuration,
        )
        .await?
        .id;

    container_api_client
        .client()
        .start_container::<String>(&id, None)
        .await?;

    Ok(())
}
