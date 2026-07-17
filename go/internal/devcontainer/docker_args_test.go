//go:build small

package devcontainer

import (
	"slices"
	"strings"
	"testing"
)

func argAfter(t *testing.T, args []string, flag string) string {
	t.Helper()
	i := slices.Index(args, flag)
	if i < 0 || i+1 >= len(args) {
		t.Fatalf("flag %q not followed by a value in %v", flag, args)
	}
	return args[i+1]
}

func argsAfterAll(args []string, flag string) []string {
	values := []string{}
	for i, a := range args {
		if a == flag && i+1 < len(args) {
			values = append(values, args[i+1])
		}
	}
	return values
}

func TestNormalizeDockerfileConfig(t *testing.T) {
	tests := []struct {
		name           string
		config         DockerfileConfig
		wantDockerfile string
		wantContext    *string
	}{
		{
			name:           "when normalize dockerfile config with only docker file then uses it",
			config:         DockerfileConfig{DockerFile: "Dockerfile.dev"},
			wantDockerfile: "Dockerfile.dev",
		},
		{
			name:           "when normalize dockerfile config with top level context then uses it",
			config:         DockerfileConfig{DockerFile: "Dockerfile", Context: ptr("..")},
			wantDockerfile: "Dockerfile",
			wantContext:    ptr(".."),
		},
		{
			name: "when normalize dockerfile config with build dockerfile then uses build dockerfile",
			config: DockerfileConfig{
				DockerFile: "Dockerfile",
				Build:      &BuildConfig{Dockerfile: ptr("Dockerfile.prod")},
			},
			wantDockerfile: "Dockerfile.prod",
		},
		{
			name: "when normalize dockerfile config with build context then uses build context",
			config: DockerfileConfig{
				DockerFile: "Dockerfile",
				Context:    ptr(".."),
				Build:      &BuildConfig{Context: ptr(".")},
			},
			wantDockerfile: "Dockerfile",
			wantContext:    ptr("."),
		},
		{
			name: "when normalize dockerfile config with build but no build dockerfile then falls back to top level",
			config: DockerfileConfig{
				DockerFile: "Dockerfile.dev",
				Build:      &BuildConfig{},
			},
			wantDockerfile: "Dockerfile.dev",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := NormalizeDockerfileConfig(&tt.config)
			if got.Dockerfile == nil || *got.Dockerfile != tt.wantDockerfile {
				t.Errorf("Dockerfile = %v, want %q", got.Dockerfile, tt.wantDockerfile)
			}
			if tt.wantContext == nil {
				if got.Context != nil {
					t.Errorf("Context = %v, want nil", got.Context)
				}
			} else if got.Context == nil || *got.Context != *tt.wantContext {
				t.Errorf("Context = %v, want %q", got.Context, *tt.wantContext)
			}
		})
	}
}

func TestContainerBuildArgs(t *testing.T) {
	devcontainerDir := "/project/.devcontainer"
	tests := []struct {
		name  string
		build BuildConfig
		check func(t *testing.T, args []string)
	}{
		{
			name:  "when container build args then includes tag",
			build: BuildConfig{},
			check: func(t *testing.T, args []string) {
				if !slices.Contains(args, "vsc-myapp") {
					t.Errorf("args = %v, want vsc-myapp included", args)
				}
			},
		},
		{
			name:  "when container build args then tag follows t flag",
			build: BuildConfig{},
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "-t"); got != "vsc-myapp" {
					t.Errorf("-t value = %q", got)
				}
			},
		},
		{
			name:  "when container build args with dockerfile then resolves against devcontainer dir",
			build: BuildConfig{Dockerfile: ptr("Dockerfile")},
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "-f"); got != "/project/.devcontainer/Dockerfile" {
					t.Errorf("-f value = %q", got)
				}
			},
		},
		{
			name:  "when container build args with no dockerfile then defaults to dockerfile",
			build: BuildConfig{},
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "-f"); got != "/project/.devcontainer/Dockerfile" {
					t.Errorf("-f value = %q", got)
				}
			},
		},
		{
			name:  "when container build args then last arg is context",
			build: BuildConfig{Context: ptr("..")},
			check: func(t *testing.T, args []string) {
				if got := args[len(args)-1]; got != "/project/.devcontainer/.." {
					t.Errorf("last arg = %q", got)
				}
			},
		},
		{
			name:  "when container build args with target then includes target",
			build: BuildConfig{Target: ptr("dev")},
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--target"); got != "dev" {
					t.Errorf("--target value = %q", got)
				}
			},
		},
		{
			name:  "when container build args with build args then includes build arg flags",
			build: BuildConfig{Args: map[string]string{"VERSION": "1.0"}},
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--build-arg"); got != "VERSION=1.0" {
					t.Errorf("--build-arg value = %q", got)
				}
			},
		},
		{
			name:  "when container build args with cache from then includes cache from flags",
			build: BuildConfig{CacheFrom: StringOrList{"myimage:latest"}},
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--cache-from"); got != "myimage:latest" {
					t.Errorf("--cache-from value = %q", got)
				}
			},
		},
		{
			name:  "when container build args with options then includes them",
			build: BuildConfig{Options: []string{"--no-cache"}},
			check: func(t *testing.T, args []string) {
				if !slices.Contains(args, "--no-cache") {
					t.Errorf("args = %v, want --no-cache included", args)
				}
			},
		},
		{
			name:  "when container build args with absolute dockerfile then uses it as is",
			build: BuildConfig{Dockerfile: ptr("/abs/Dockerfile")},
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "-f"); got != "/abs/Dockerfile" {
					t.Errorf("-f value = %q", got)
				}
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tt.check(t, ContainerBuildArgs(&tt.build, devcontainerDir, "vsc-myapp"))
		})
	}
}

func TestContainerStartArgs(t *testing.T) {
	tests := []struct {
		name            string
		overrideCommand *bool
		entrypoint      []string
		cmd             []string
		check           func(t *testing.T, args []string)
	}{
		{
			name: "when container start args with override command unset then returns loop script",
			check: func(t *testing.T, args []string) {
				if args[0] != "-c" || args[2] != "-" {
					t.Errorf("args = %v", args)
				}
				if !strings.Contains(args[1], "while sleep 1") {
					t.Errorf("script = %q", args[1])
				}
			},
		},
		{
			name:            "when container start args with override command true then returns loop script",
			overrideCommand: ptr(true),
			check: func(t *testing.T, args []string) {
				if args[0] != "-c" || args[2] != "-" {
					t.Errorf("args = %v", args)
				}
				if !strings.Contains(args[1], "while sleep 1") {
					t.Errorf("script = %q", args[1])
				}
			},
		},
		{
			name:            "when container start args with override command false then appends image entrypoint and cmd",
			overrideCommand: ptr(false),
			entrypoint:      []string{"/entrypoint.sh"},
			cmd:             []string{"--flag"},
			check: func(t *testing.T, args []string) {
				if args[2] != "-" || args[3] != "/entrypoint.sh" || args[4] != "--flag" {
					t.Errorf("args = %v", args)
				}
			},
		},
		{
			name:            "when container start args with override command true then does not append image cmd",
			overrideCommand: ptr(true),
			entrypoint:      []string{"/entrypoint.sh"},
			cmd:             []string{"--flag"},
			check: func(t *testing.T, args []string) {
				if len(args) != 3 {
					t.Errorf("len(args) = %d, want 3: %v", len(args), args)
				}
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tt.check(t, ContainerStartArgs(tt.overrideCommand, tt.entrypoint, tt.cmd))
		})
	}
}

func TestContainerRunOptions(t *testing.T) {
	tests := []struct {
		name           string
		common         CommonConfig
		appPort        AppPorts
		runArgs        []string
		workspaceMount *string
		localFolder    string
		configFile     string
		check          func(t *testing.T, args []string)
	}{
		{
			name:        "when container run options then includes detach flag",
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if !slices.Contains(args, "-d") {
					t.Errorf("args = %v, want -d included", args)
				}
			},
		},
		{
			name:        "when container run options then includes local folder label",
			localFolder: "/home/user/project",
			configFile:  "/home/user/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				labels := argsAfterAll(args, "--label")
				if !slices.Contains(labels, "devcontainer.local_folder=/home/user/project") {
					t.Errorf("labels = %v", labels)
				}
			},
		},
		{
			name:        "when container run options then includes config file label",
			localFolder: "/home/user/project",
			configFile:  "/home/user/project/.devcontainer/server/devcontainer.json",
			check: func(t *testing.T, args []string) {
				labels := argsAfterAll(args, "--label")
				if !slices.Contains(labels, "devcontainer.config_file=/home/user/project/.devcontainer/server/devcontainer.json") {
					t.Errorf("labels = %v", labels)
				}
			},
		},
		{
			name:        "when container run options with no workspace folder then uses basename as default",
			localFolder: "/home/user/myproject",
			configFile:  "/home/user/myproject/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "-w"); got != "/workspaces/myproject" {
					t.Errorf("-w value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with workspace folder then uses it as workdir",
			common:      CommonConfig{WorkspaceFolder: ptr("/workspace")},
			localFolder: "/home/user/project",
			configFile:  "/home/user/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "-w"); got != "/workspace" {
					t.Errorf("-w value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with workspace folder then mount target is still basename",
			common:      CommonConfig{WorkspaceFolder: ptr("/workspaces/foo")},
			localFolder: "/home/user/myproject",
			configFile:  "/home/user/myproject/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--mount"); got != "type=bind,source=/home/user/myproject,target=/workspaces/myproject" {
					t.Errorf("--mount value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with no workspace mount then uses bind mount to workspace folder",
			localFolder: "/home/user/myproject",
			configFile:  "/home/user/myproject/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--mount"); got != "type=bind,source=/home/user/myproject,target=/workspaces/myproject" {
					t.Errorf("--mount value = %q", got)
				}
			},
		},
		{
			name:           "when container run options with workspace mount then uses it",
			workspaceMount: ptr("source=/custom,target=/workspace,type=bind"),
			localFolder:    "/project",
			configFile:     "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--mount"); got != "source=/custom,target=/workspace,type=bind" {
					t.Errorf("--mount value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with container env then includes env flags",
			common:      CommonConfig{ContainerEnv: map[string]string{"RUST_LOG": "debug"}},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--env"); got != "RUST_LOG=debug" {
					t.Errorf("--env value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with container user then includes user flag",
			common:      CommonConfig{ContainerUser: ptr("vscode")},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--user"); got != "vscode" {
					t.Errorf("--user value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with init true then includes init flag",
			common:      CommonConfig{Init: ptr(true)},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if !slices.Contains(args, "--init") {
					t.Errorf("args = %v, want --init included", args)
				}
			},
		},
		{
			name:        "when container run options with init false then excludes init flag",
			common:      CommonConfig{Init: ptr(false)},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if slices.Contains(args, "--init") {
					t.Errorf("args = %v, want --init excluded", args)
				}
			},
		},
		{
			name:        "when container run options with privileged true then includes privileged flag",
			common:      CommonConfig{Privileged: ptr(true)},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if !slices.Contains(args, "--privileged") {
					t.Errorf("args = %v, want --privileged included", args)
				}
			},
		},
		{
			name:        "when container run options with cap add then includes cap add flags",
			common:      CommonConfig{CapAdd: []string{"SYS_PTRACE"}},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--cap-add"); got != "SYS_PTRACE" {
					t.Errorf("--cap-add value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with security opt then includes security opt flags",
			common:      CommonConfig{SecurityOpt: []string{"seccomp=unconfined"}},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "--security-opt"); got != "seccomp=unconfined" {
					t.Errorf("--security-opt value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with app port then includes p flags",
			appPort:     AppPorts{"127.0.0.1:8080:8080"},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if got := argAfter(t, args, "-p"); got != "127.0.0.1:8080:8080" {
					t.Errorf("-p value = %q", got)
				}
			},
		},
		{
			name:        "when container run options with multiple app ports then includes all p flags",
			appPort:     AppPorts{"127.0.0.1:3000:3000", "4000:80"},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				ports := argsAfterAll(args, "-p")
				want := []string{"127.0.0.1:3000:3000", "4000:80"}
				if !slices.Equal(ports, want) {
					t.Errorf("-p values = %v, want %v", ports, want)
				}
			},
		},
		{
			name:        "when container run options with run args then includes them",
			runArgs:     []string{"--network=host"},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				if !slices.Contains(args, "--network=host") {
					t.Errorf("args = %v, want --network=host included", args)
				}
			},
		},
		{
			name:        "when container run options with additional mounts then includes mount flags",
			common:      CommonConfig{Mounts: []any{"source=/host/data,target=/container/data,type=bind"}},
			localFolder: "/project",
			configFile:  "/project/.devcontainer/devcontainer.json",
			check: func(t *testing.T, args []string) {
				mounts := argsAfterAll(args, "--mount")
				if mounts[len(mounts)-1] != "source=/host/data,target=/container/data,type=bind" {
					t.Errorf("--mount values = %v", mounts)
				}
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			args := ContainerRunOptions(&tt.common, tt.appPort, tt.runArgs, tt.workspaceMount, tt.localFolder, tt.configFile)
			tt.check(t, args)
		})
	}
}
