package main

import (
	"fmt"
	"os"

	"github.com/toms74209200/cashyyc/internal/cashyyc"
)

func main() {
	if err := cashyyc.Run(os.Args); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %s\n", err)
		os.Exit(1)
	}
}
