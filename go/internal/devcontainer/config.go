package devcontainer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"slices"
	"strconv"
)

type AppPorts []string

func (p *AppPorts) UnmarshalJSON(data []byte) error {
	dec := json.NewDecoder(bytes.NewReader(data))
	dec.UseNumber()
	var v any
	if err := dec.Decode(&v); err != nil {
		return err
	}
	switch x := v.(type) {
	case nil:
		*p = AppPorts{}
	case json.Number, string:
		port, err := normalizeAppPort(x)
		if err != nil {
			return err
		}
		*p = AppPorts{port}
	case []any:
		ports := make(AppPorts, 0, len(x))
		for _, e := range x {
			port, err := normalizeAppPort(e)
			if err != nil {
				return err
			}
			ports = append(ports, port)
		}
		*p = ports
	default:
		return fmt.Errorf("appPort must be a number, string, or array: %v", v)
	}
	return nil
}

func normalizeAppPort(v any) (string, error) {
	switch x := v.(type) {
	case json.Number:
		n, err := strconv.ParseUint(x.String(), 10, 64)
		if err != nil {
			return "", err
		}
		return fmt.Sprintf("127.0.0.1:%d:%d", n, n), nil
	case string:
		return x, nil
	}
	return "", fmt.Errorf("appPort element must be a number or string: %v", v)
}

type StringOrList []string

func (s *StringOrList) UnmarshalJSON(data []byte) error {
	var v any
	if err := json.Unmarshal(data, &v); err != nil {
		return err
	}
	switch x := v.(type) {
	case nil:
		*s = nil
	case string:
		*s = StringOrList{x}
	case []any:
		list := make(StringOrList, 0, len(x))
		for _, e := range x {
			str, ok := e.(string)
			if !ok {
				return fmt.Errorf("expected string in list: %v", e)
			}
			list = append(list, str)
		}
		*s = list
	default:
		return fmt.Errorf("expected string or list of strings: %v", v)
	}
	return nil
}

type UserEnvProbe string

const (
	UserEnvProbeNone                  UserEnvProbe = "none"
	UserEnvProbeLoginInteractiveShell UserEnvProbe = "loginInteractiveShell"
	UserEnvProbeInteractiveShell      UserEnvProbe = "interactiveShell"
	UserEnvProbeLoginShell            UserEnvProbe = "loginShell"
)

func (p *UserEnvProbe) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}
	switch v := UserEnvProbe(s); v {
	case UserEnvProbeNone, UserEnvProbeLoginInteractiveShell, UserEnvProbeInteractiveShell, UserEnvProbeLoginShell:
		*p = v
		return nil
	}
	return fmt.Errorf("unknown userEnvProbe: %q", s)
}

type WaitFor string

const (
	WaitForInitializeCommand    WaitFor = "initializeCommand"
	WaitForOnCreateCommand      WaitFor = "onCreateCommand"
	WaitForUpdateContentCommand WaitFor = "updateContentCommand"
	WaitForPostCreateCommand    WaitFor = "postCreateCommand"
	WaitForPostStartCommand     WaitFor = "postStartCommand"
	WaitForPostAttachCommand    WaitFor = "postAttachCommand"
)

var waitForChain = []WaitFor{
	WaitForInitializeCommand,
	WaitForOnCreateCommand,
	WaitForUpdateContentCommand,
	WaitForPostCreateCommand,
	WaitForPostStartCommand,
	WaitForPostAttachCommand,
}

func (w *WaitFor) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}
	if !slices.Contains(waitForChain, WaitFor(s)) {
		return fmt.Errorf("unknown waitFor: %q", s)
	}
	*w = WaitFor(s)
	return nil
}

func (w WaitFor) Requires(cmd WaitFor) bool {
	c := slices.Index(waitForChain, cmd)
	s := slices.Index(waitForChain, w)
	return c >= 0 && s >= 0 && c <= s
}

type PortAttributes struct {
	Label           *string `json:"label"`
	OnAutoForward   *string `json:"onAutoForward"`
	ElevateIfNeeded *bool   `json:"elevateIfNeeded"`
}

type HostRequirements struct {
	Cpus    *uint32 `json:"cpus"`
	Memory  *string `json:"memory"`
	Storage *string `json:"storage"`
	Gpu     any     `json:"gpu"`
}

type CommonConfig struct {
	Name                        *string                   `json:"name"`
	ForwardPorts                []any                     `json:"forwardPorts"`
	PortsAttributes             map[string]PortAttributes `json:"portsAttributes"`
	OtherPortsAttributes        *PortAttributes           `json:"otherPortsAttributes"`
	OverrideCommand             *bool                     `json:"overrideCommand"`
	InitializeCommand           any                       `json:"initializeCommand"`
	OnCreateCommand             any                       `json:"onCreateCommand"`
	UpdateContentCommand        any                       `json:"updateContentCommand"`
	PostCreateCommand           any                       `json:"postCreateCommand"`
	PostStartCommand            any                       `json:"postStartCommand"`
	PostAttachCommand           any                       `json:"postAttachCommand"`
	WaitFor                     *WaitFor                  `json:"waitFor"`
	WorkspaceFolder             *string                   `json:"workspaceFolder"`
	Mounts                      []any                     `json:"mounts"`
	ContainerEnv                map[string]string         `json:"containerEnv"`
	ContainerUser               *string                   `json:"containerUser"`
	Init                        *bool                     `json:"init"`
	Privileged                  *bool                     `json:"privileged"`
	CapAdd                      []string                  `json:"capAdd"`
	SecurityOpt                 []string                  `json:"securityOpt"`
	RemoteEnv                   map[string]*string        `json:"remoteEnv"`
	RemoteUser                  *string                   `json:"remoteUser"`
	UpdateRemoteUserUID         *bool                     `json:"updateRemoteUserUID"`
	UserEnvProbe                *UserEnvProbe             `json:"userEnvProbe"`
	Features                    map[string]any            `json:"features"`
	OverrideFeatureInstallOrder []string                  `json:"overrideFeatureInstallOrder"`
	HostRequirements            *HostRequirements         `json:"hostRequirements"`
	Customizations              map[string]any            `json:"customizations"`
}

type BuildConfig struct {
	Dockerfile *string           `json:"dockerfile"`
	Context    *string           `json:"context"`
	Target     *string           `json:"target"`
	Args       map[string]string `json:"args"`
	CacheFrom  StringOrList      `json:"cacheFrom"`
	Options    []string          `json:"options"`
}

type DockerComposeConfig struct {
	DockerComposeFile StringOrList `json:"dockerComposeFile"`
	Service           string       `json:"service"`
	WorkspaceFolder   string       `json:"workspaceFolder"`
	RunServices       []string     `json:"runServices"`
	ShutdownAction    *string      `json:"shutdownAction"`
	CommonConfig
}

type DockerfileConfig struct {
	DockerFile     string       `json:"dockerFile"`
	Context        *string      `json:"context"`
	Build          *BuildConfig `json:"build"`
	AppPort        AppPorts     `json:"appPort"`
	RunArgs        []string     `json:"runArgs"`
	WorkspaceMount *string      `json:"workspaceMount"`
	ShutdownAction *string      `json:"shutdownAction"`
	CommonConfig
}

type DockerfileBuildConfig struct {
	Build          BuildConfig `json:"build"`
	AppPort        AppPorts    `json:"appPort"`
	RunArgs        []string    `json:"runArgs"`
	WorkspaceMount *string     `json:"workspaceMount"`
	ShutdownAction *string     `json:"shutdownAction"`
	CommonConfig
}

type ImageConfig struct {
	Image          string   `json:"image"`
	AppPort        AppPorts `json:"appPort"`
	RunArgs        []string `json:"runArgs"`
	WorkspaceMount *string  `json:"workspaceMount"`
	ShutdownAction *string  `json:"shutdownAction"`
	CommonConfig
}

type Config interface {
	Common() *CommonConfig
	ResolveWorkspaceFolder(cwd string) string
}

func (c *ImageConfig) Common() *CommonConfig           { return &c.CommonConfig }
func (c *DockerfileConfig) Common() *CommonConfig      { return &c.CommonConfig }
func (c *DockerfileBuildConfig) Common() *CommonConfig { return &c.CommonConfig }
func (c *DockerComposeConfig) Common() *CommonConfig   { return &c.CommonConfig }

func (c *ImageConfig) ResolveWorkspaceFolder(cwd string) string {
	return resolveWorkspaceFolder(c.WorkspaceFolder, cwd)
}

func (c *DockerfileConfig) ResolveWorkspaceFolder(cwd string) string {
	return resolveWorkspaceFolder(c.WorkspaceFolder, cwd)
}

func (c *DockerfileBuildConfig) ResolveWorkspaceFolder(cwd string) string {
	return resolveWorkspaceFolder(c.WorkspaceFolder, cwd)
}

func (c *DockerComposeConfig) ResolveWorkspaceFolder(cwd string) string {
	return resolveWorkspaceFolder(&c.WorkspaceFolder, cwd)
}

func resolveWorkspaceFolder(explicit *string, cwd string) string {
	defaultFolder := "/workspaces/" + pathBasename(cwd)
	raw := defaultFolder
	if explicit != nil {
		raw = *explicit
	}
	return ExpandVariables(raw, cwd, defaultFolder, map[string]string{})
}
