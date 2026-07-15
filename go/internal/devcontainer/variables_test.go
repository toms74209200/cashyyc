//go:build small

package devcontainer

import (
	"math/rand/v2"
	"os"
	"testing"
)

const randomChars = "abcdefghijklmnopqrstuvwxyz"

func randomName() string {
	b := make([]byte, 8)
	for i := range b {
		b[i] = randomChars[rand.IntN(len(randomChars))]
	}
	return string(b)
}

func TestExpandVariables(t *testing.T) {
	name := randomName()
	localFolder := "/home/user/" + name
	containerFolder := "/workspaces/" + name
	pathEnv := os.Getenv("PATH")
	tests := []struct {
		name            string
		value           string
		localFolder     string
		containerFolder string
		containerEnv    map[string]string
		want            string
	}{
		{
			name:            "when expand variables with local workspace folder then replaces with local folder",
			value:           "${localWorkspaceFolder}/data",
			localFolder:     localFolder,
			containerFolder: containerFolder,
			want:            localFolder + "/data",
		},
		{
			name:            "when expand variables with local workspace folder basename then replaces with basename",
			value:           "source=${localWorkspaceFolderBasename}",
			localFolder:     localFolder,
			containerFolder: containerFolder,
			want:            "source=" + name,
		},
		{
			name:            "when expand variables with container workspace folder then replaces with container path",
			value:           "${containerWorkspaceFolder}/build",
			localFolder:     localFolder,
			containerFolder: containerFolder,
			want:            containerFolder + "/build",
		},
		{
			name:            "when expand variables with container workspace folder basename then replaces with basename",
			value:           "target=${containerWorkspaceFolderBasename}",
			localFolder:     localFolder,
			containerFolder: containerFolder,
			want:            "target=" + name,
		},
		{
			name:            "when expand variables with local workspace folder basename with trailing slash then replaces with basename",
			value:           "source=${localWorkspaceFolderBasename}",
			localFolder:     localFolder + "/",
			containerFolder: containerFolder,
			want:            "source=" + name,
		},
		{
			name:            "when expand variables with container workspace folder basename with trailing slash then replaces with basename",
			value:           "target=${containerWorkspaceFolderBasename}",
			localFolder:     localFolder,
			containerFolder: containerFolder + "/",
			want:            "target=" + name,
		},
		{
			name:            "when expand variables with no variables then returns unchanged",
			value:           "type=bind,source=/host,target=/container",
			localFolder:     localFolder,
			containerFolder: containerFolder,
			want:            "type=bind,source=/host,target=/container",
		},
		{
			name:            "when expand variables with root path as local workspace folder basename then returns empty",
			value:           "${localWorkspaceFolderBasename}",
			localFolder:     "/",
			containerFolder: "/workspaces/project",
			want:            "",
		},
		{
			name:            "when expand variables with root path as container workspace folder basename then returns empty",
			value:           "${containerWorkspaceFolderBasename}",
			localFolder:     localFolder,
			containerFolder: "/",
			want:            "",
		},
		{
			name:            "when expand variables with multiple variables then replaces all",
			value:           "type=bind,source=${localWorkspaceFolder},target=${containerWorkspaceFolder}",
			localFolder:     localFolder,
			containerFolder: containerFolder,
			want:            "type=bind,source=" + localFolder + ",target=" + containerFolder,
		},
		{
			name:            "when expand variables with devcontainer id then expands to fnv1a hash of local folder",
			value:           "${devcontainerId}",
			localFolder:     "/home/user/project",
			containerFolder: "/workspaces/project",
			want:            "f8a71a04e8340307",
		},
		{
			name:            "when expand variables with local env then expands to env value",
			value:           "${localEnv:PATH}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            pathEnv,
		},
		{
			name:            "when expand variables with missing local env then expands to empty",
			value:           "prefix_${localEnv:__CYYC_NO_SUCH_VAR__}_suffix",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "prefix__suffix",
		},
		{
			name:            "when expand variables with multiple local env then expands all",
			value:           "${localEnv:PATH}:${localEnv:__CYYC_NO_SUCH_VAR__}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            pathEnv + ":",
		},
		{
			name:            "when expand variables with unclosed local env brace then treats literally",
			value:           "${localEnv:PATH",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "${localEnv:PATH",
		},
		{
			name:            "when expand variables with local env with default and env set then uses env value",
			value:           "${localEnv:PATH:fallback}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            pathEnv,
		},
		{
			name:            "when expand variables with local env with default and env missing then uses default",
			value:           "${localEnv:__CYYC_NO_SUCH_VAR__:mydefault}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "mydefault",
		},
		{
			name:            "when expand variables with container env then expands to container env value",
			value:           "${containerEnv:HOME}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			containerEnv:    map[string]string{"HOME": "/root"},
			want:            "/root",
		},
		{
			name:            "when expand variables with missing container env then expands to empty",
			value:           "prefix_${containerEnv:__NO_SUCH_VAR__}_suffix",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "prefix__suffix",
		},
		{
			name:            "when expand variables with container env with default and env missing then uses default",
			value:           "${containerEnv:__NO_SUCH_VAR__:mydefault}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "mydefault",
		},
		{
			name:            "when expand variables with container env with default and env present then uses container value",
			value:           "${containerEnv:MYVAR:fallback}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			containerEnv:    map[string]string{"MYVAR": "containervalue"},
			want:            "containervalue",
		},
		{
			name:            "when expand variables with multiple container env then expands all",
			value:           "${containerEnv:FOO}:${containerEnv:__NO_SUCH_VAR__}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			containerEnv:    map[string]string{"FOO": "foo_val"},
			want:            "foo_val:",
		},
		{
			name:            "when expand variables with unclosed container env brace then treats literally",
			value:           "${containerEnv:HOME",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "${containerEnv:HOME",
		},
		{
			name:            "when expand variables with unknown variable then treats literally",
			value:           "${unknownVariable}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "${unknownVariable}",
		},
		{
			name:            "when expand variables with unknown scope then treats literally",
			value:           "${unknownScope:NAME}",
			localFolder:     "/home/user",
			containerFolder: "/workspaces/x",
			want:            "${unknownScope:NAME}",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			containerEnv := tt.containerEnv
			if containerEnv == nil {
				containerEnv = map[string]string{}
			}
			got := ExpandVariables(tt.value, tt.localFolder, tt.containerFolder, containerEnv)
			if got != tt.want {
				t.Errorf("ExpandVariables(%q) = %q, want %q", tt.value, got, tt.want)
			}
		})
	}
}
