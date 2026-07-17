package devcontainer

import (
	"fmt"
	"strings"
)

func NormalizeDockerfileConfig(config *DockerfileConfig) BuildConfig {
	build := BuildConfig{}
	if config.Build != nil {
		build = *config.Build
	}
	if build.Dockerfile == nil {
		dockerFile := config.DockerFile
		build.Dockerfile = &dockerFile
	}
	if build.Context == nil {
		build.Context = config.Context
	}
	return build
}

func ContainerBuildArgs(build *BuildConfig, devcontainerDir string, tag string) []string {
	dockerfile := "Dockerfile"
	if build.Dockerfile != nil {
		dockerfile = *build.Dockerfile
	}
	context := "."
	if build.Context != nil {
		context = *build.Context
	}

	args := []string{
		"-t", tag,
		"-f", joinPath(devcontainerDir, dockerfile),
	}

	if build.Target != nil {
		args = append(args, "--target", *build.Target)
	}
	for k, v := range build.Args {
		args = append(args, "--build-arg", fmt.Sprintf("%s=%s", k, v))
	}
	for _, c := range build.CacheFrom {
		args = append(args, "--cache-from", c)
	}
	args = append(args, build.Options...)

	args = append(args, joinPath(devcontainerDir, context))
	return args
}

const containerLoopScript = "echo Container started\ntrap \"exit 0\" 15\nexec \"$@\"\nwhile sleep 1 & wait $!; do :; done"

func ContainerStartArgs(overrideCommand *bool, imageEntrypoint []string, imageCmd []string) []string {
	args := []string{"-c", containerLoopScript, "-"}
	if overrideCommand != nil && !*overrideCommand {
		args = append(args, imageEntrypoint...)
		args = append(args, imageCmd...)
	}
	return args
}

func ContainerRunOptions(
	common *CommonConfig,
	appPort AppPorts,
	runArgs []string,
	workspaceMount *string,
	localFolder string,
	configFile string,
) []string {
	args := []string{
		"-d",
		"--label", fmt.Sprintf("devcontainer.local_folder=%s", localFolder),
		"--label", fmt.Sprintf("devcontainer.config_file=%s", configFile),
	}

	emptyEnv := map[string]string{}
	defaultWorkspaceFolder := "/workspaces/" + pathBasename(localFolder)
	rawWorkspaceFolder := defaultWorkspaceFolder
	if common.WorkspaceFolder != nil {
		rawWorkspaceFolder = *common.WorkspaceFolder
	}
	workspaceFolder := ExpandVariables(rawWorkspaceFolder, localFolder, defaultWorkspaceFolder, emptyEnv)

	mount := fmt.Sprintf("type=bind,source=%s,target=%s", localFolder, defaultWorkspaceFolder)
	if workspaceMount != nil {
		mount = *workspaceMount
	}
	args = append(args, "--mount", mount)
	args = append(args, "-w", workspaceFolder)

	for _, m := range common.Mounts {
		if s, ok := m.(string); ok {
			args = append(args, "--mount", ExpandVariables(s, localFolder, workspaceFolder, emptyEnv))
		}
	}

	for key, value := range common.ContainerEnv {
		expanded := ExpandVariables(value, localFolder, workspaceFolder, emptyEnv)
		args = append(args, "--env", fmt.Sprintf("%s=%s", key, expanded))
	}

	if common.ContainerUser != nil {
		args = append(args, "--user", ExpandVariables(*common.ContainerUser, localFolder, workspaceFolder, emptyEnv))
	}

	if common.Init != nil && *common.Init {
		args = append(args, "--init")
	}

	if common.Privileged != nil && *common.Privileged {
		args = append(args, "--privileged")
	}

	for _, cap := range common.CapAdd {
		args = append(args, "--cap-add", cap)
	}

	for _, opt := range common.SecurityOpt {
		args = append(args, "--security-opt", opt)
	}

	for _, port := range appPort {
		args = append(args, "-p", port)
	}

	for _, a := range runArgs {
		args = append(args, ExpandVariables(a, localFolder, workspaceFolder, emptyEnv))
	}

	return args
}

func joinPath(base string, path string) string {
	if strings.HasPrefix(path, "/") {
		return path
	}
	return strings.TrimSuffix(base, "/") + "/" + path
}
