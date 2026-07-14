package devcontainer

import (
	"fmt"
	"hash/fnv"
	"os"
	"path/filepath"
	"strings"
)

func ExpandVariables(value string, localFolder string, containerWorkspaceFolder string, containerEnv map[string]string) string {
	first, rest, found := strings.Cut(value, "${")
	if !found {
		return value
	}
	var b strings.Builder
	b.WriteString(first)
	for _, part := range strings.Split(rest, "${") {
		content, tail, closed := strings.Cut(part, "}")
		if !closed {
			b.WriteString("${")
			b.WriteString(part)
			continue
		}
		resolved, ok := resolveVariable(content, localFolder, containerWorkspaceFolder, containerEnv)
		if !ok {
			resolved = "${" + content + "}"
		}
		b.WriteString(resolved)
		b.WriteString(tail)
	}
	return b.String()
}

func resolveVariable(content string, localFolder string, containerWorkspaceFolder string, containerEnv map[string]string) (string, bool) {
	switch content {
	case "localWorkspaceFolder":
		return localFolder, true
	case "localWorkspaceFolderBasename":
		return pathBasename(localFolder), true
	case "containerWorkspaceFolder":
		return containerWorkspaceFolder, true
	case "containerWorkspaceFolderBasename":
		return pathBasename(containerWorkspaceFolder), true
	case "devcontainerId":
		h := fnv.New64a()
		h.Write([]byte(localFolder))
		return fmt.Sprintf("%016x", h.Sum64()), true
	}
	scope, rest, found := strings.Cut(content, ":")
	if !found {
		return "", false
	}
	name, defaultValue, _ := strings.Cut(rest, ":")
	switch scope {
	case "localEnv":
		if v, ok := os.LookupEnv(name); ok {
			return v, true
		}
		return defaultValue, true
	case "containerEnv":
		if v, ok := containerEnv[name]; ok {
			return v, true
		}
		return defaultValue, true
	}
	return "", false
}

func pathBasename(p string) string {
	switch b := filepath.Base(p); b {
	case "/", ".", "..":
		return ""
	default:
		return b
	}
}
