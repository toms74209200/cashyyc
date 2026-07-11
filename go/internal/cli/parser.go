package cli

type Command interface {
	command()
}

type Shell struct {
	Name string
}

type Stop struct {
	Name string
}

type Down struct {
	Name string
}

type Ps struct {
	Name string
}

type New struct{}

type Help struct{}

type Version struct{}

type Unknown struct {
	Message string
}

func (Shell) command()   {}
func (Stop) command()    {}
func (Down) command()    {}
func (Ps) command()      {}
func (New) command()     {}
func (Help) command()    {}
func (Version) command() {}
func (Unknown) command() {}

func ParseArgs(args []string) Command {
	if len(args) < 2 {
		return Unknown{Message: "no command"}
	}
	name := ""
	if len(args) > 2 {
		name = args[2]
	}
	switch args[1] {
	case "shell":
		return Shell{Name: name}
	case "stop":
		return Stop{Name: name}
	case "down":
		return Down{Name: name}
	case "ps":
		return Ps{Name: name}
	case "new":
		return New{}
	case "help", "--help", "-h":
		return Help{}
	case "version", "--version", "-V":
		return Version{}
	default:
		return Unknown{Message: args[1]}
	}
}
