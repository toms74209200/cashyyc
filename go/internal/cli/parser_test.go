//go:build small

package cli

import (
	"math/rand/v2"
	"testing"
)

const randomChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"

func randomString(n int) string {
	b := make([]byte, n)
	for i := range b {
		b[i] = randomChars[rand.IntN(len(randomChars))]
	}
	return string(b)
}

func TestParseArgs(t *testing.T) {
	name := randomString(8)
	tests := []struct {
		name string
		args []string
		want Command
	}{
		{
			name: "when parse args with shell command then returns shell with no name",
			args: []string{"cyyc", "shell"},
			want: Shell{},
		},
		{
			name: "when parse args with shell and environment name then returns shell with name",
			args: []string{"cyyc", "shell", name},
			want: Shell{Name: name},
		},
		{
			name: "when parse args with stop command then returns stop with no name",
			args: []string{"cyyc", "stop"},
			want: Stop{},
		},
		{
			name: "when parse args with stop and environment name then returns stop with name",
			args: []string{"cyyc", "stop", name},
			want: Stop{Name: name},
		},
		{
			name: "when parse args with down command then returns down with no name",
			args: []string{"cyyc", "down"},
			want: Down{},
		},
		{
			name: "when parse args with down and environment name then returns down with name",
			args: []string{"cyyc", "down", name},
			want: Down{Name: name},
		},
		{
			name: "when parse args with ps command then returns ps with no name",
			args: []string{"cyyc", "ps"},
			want: Ps{},
		},
		{
			name: "when parse args with ps and environment name then returns ps with name",
			args: []string{"cyyc", "ps", name},
			want: Ps{Name: name},
		},
		{
			name: "when parse args with new command then returns new",
			args: []string{"cyyc", "new"},
			want: New{},
		},
		{
			name: "when parse args with new command and extra arg then returns new",
			args: []string{"cyyc", "new", name},
			want: New{},
		},
		{
			name: "when parse args with help command then returns help",
			args: []string{"cyyc", "help"},
			want: Help{},
		},
		{
			name: "when parse args with help flag then returns help",
			args: []string{"cyyc", "--help"},
			want: Help{},
		},
		{
			name: "when parse args with short help flag then returns help",
			args: []string{"cyyc", "-h"},
			want: Help{},
		},
		{
			name: "when parse args with help command and extra arg then returns help",
			args: []string{"cyyc", "help", name},
			want: Help{},
		},
		{
			name: "when parse args with version command then returns version",
			args: []string{"cyyc", "version"},
			want: Version{},
		},
		{
			name: "when parse args with version flag then returns version",
			args: []string{"cyyc", "--version"},
			want: Version{},
		},
		{
			name: "when parse args with short version flag then returns version",
			args: []string{"cyyc", "-V"},
			want: Version{},
		},
		{
			name: "when parse args with version command and extra arg then returns version",
			args: []string{"cyyc", "version", name},
			want: Version{},
		},
		{
			name: "when parse args with unknown command then returns unknown",
			args: []string{"cyyc", name},
			want: Unknown{Message: name},
		},
		{
			name: "when parse args with unknown command and extra arg then returns unknown",
			args: []string{"cyyc", name, randomString(8)},
			want: Unknown{Message: name},
		},
		{
			name: "when parse args with program name only then returns unknown",
			args: []string{"cyyc"},
			want: Unknown{Message: "no command"},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ParseArgs(tt.args)
			if got != tt.want {
				t.Errorf("ParseArgs(%v) = %#v, want %#v", tt.args, got, tt.want)
			}
		})
	}
}
