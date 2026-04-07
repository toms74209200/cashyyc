Feature: cyyc shell

  Scenario: Start and enter the default environment when it is not running
    Given a default devcontainer environment exists
    And the container is not running
    When running "cyyc shell"
    Then the container is running

  Scenario: Start and enter a named environment when it is not running
    Given a "python" devcontainer environment exists
    And the container is not running
    When running "cyyc shell python"
    Then the container is running

  Scenario: No default environment exists
    Given no default devcontainer environment exists
    And a "python" devcontainer environment exists
    When running "cyyc shell"
    Then the available environment names are printed
    And the command exits with a non-zero status
