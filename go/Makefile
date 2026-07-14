.PHONY: all build test test-cover lint fmt clean help

all: lint test build

build: ## Build the project
	go build -v ./...

test: ## Run small size tests
	go test -tags=small -v ./...

test-cover: ## Run tests with coverage
	go test -tags=small -race -coverprofile=coverage.txt -covermode=atomic `cat scripts/coverage_pkgs.txt`
	go tool cover -func=coverage.txt

lint: ## Run lint
	golangci-lint run

fmt: ## Run go fmt and goimports
	go fmt ./...
	goimports -w .

clean: ## Clean build artifacts
	rm -f coverage.txt
	rm -f cyyc

help: ## Display this help screen
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
