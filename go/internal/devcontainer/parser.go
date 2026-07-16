package devcontainer

import (
	"encoding/json"
	"strings"
)

func ParseConfig(content string) Config {
	clean := stripTrailingCommas(stripComments(content))
	var raw map[string]json.RawMessage
	if err := json.Unmarshal([]byte(clean), &raw); err != nil {
		return nil
	}
	data := []byte(clean)
	if _, ok := raw["dockerComposeFile"]; ok {
		var c DockerComposeConfig
		if err := json.Unmarshal(data, &c); err != nil {
			return nil
		}
		if _, ok := raw["service"]; !ok {
			return nil
		}
		if _, ok := raw["workspaceFolder"]; !ok {
			return nil
		}
		return &c
	}
	if _, ok := raw["dockerFile"]; ok {
		var c DockerfileConfig
		if err := json.Unmarshal(data, &c); err != nil {
			return nil
		}
		return &c
	}
	if _, ok := raw["build"]; ok {
		var c DockerfileBuildConfig
		if err := json.Unmarshal(data, &c); err != nil {
			return nil
		}
		return &c
	}
	if _, ok := raw["image"]; ok {
		var c ImageConfig
		if err := json.Unmarshal(data, &c); err != nil {
			return nil
		}
		return &c
	}
	return nil
}

func stripComments(content string) string {
	var b strings.Builder
	b.Grow(len(content))
	inString := false
	i := 0
	n := len(content)
	for i < n {
		c := content[i]
		switch {
		case inString && c == '\\':
			b.WriteByte(c)
			i++
			if i < n {
				b.WriteByte(content[i])
				i++
			}
		case c == '"':
			inString = !inString
			b.WriteByte(c)
			i++
		case !inString && c == '/' && i+1 < n && content[i+1] == '/':
			i += 2
			for i < n && content[i] != '\n' {
				i++
			}
			if i < n {
				b.WriteByte('\n')
				i++
			}
		case !inString && c == '/' && i+1 < n && content[i+1] == '*':
			i += 2
			for i < n {
				if content[i] == '*' && i+1 < n && content[i+1] == '/' {
					i += 2
					break
				}
				i++
			}
		default:
			b.WriteByte(c)
			i++
		}
	}
	return b.String()
}

func stripTrailingCommas(content string) string {
	var b strings.Builder
	b.Grow(len(content))
	inString := false
	i := 0
	n := len(content)
	for i < n {
		c := content[i]
		switch {
		case inString && c == '\\':
			b.WriteByte(c)
			i++
			if i < n {
				b.WriteByte(content[i])
				i++
			}
		case c == '"':
			inString = !inString
			b.WriteByte(c)
			i++
		case !inString && c == ',':
			j := i + 1
			for j < n && isASCIIWhitespace(content[j]) {
				j++
			}
			if j >= n || (content[j] != '}' && content[j] != ']') {
				b.WriteByte(',')
			}
			b.WriteString(content[i+1 : j])
			i = j
		default:
			b.WriteByte(c)
			i++
		}
	}
	return b.String()
}

func isASCIIWhitespace(c byte) bool {
	return c == ' ' || c == '\t' || c == '\n' || c == '\f' || c == '\r'
}
