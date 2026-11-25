# Panel
panel-week = W
panel-month = M
panel-error = Err

# Popup
popup-rolling = 5h
popup-week = W
popup-month = M
popup-mcp = MCP · M
resets-in = Resets in { $d }
updated-at = Updated: { $t }
error-line = Error: { $e }
no-data = No data
quit = Quit
settings = Settings

# Errors
hint-no-key = No API key configured

# Durations (number included for composition in Rust)
duration-day =
    { $n ->
        [one] 1 day
       *[other] { $n } days
    }
duration-hour =
    { $n ->
        [one] 1 hour
       *[other] { $n } hours
    }
duration-minute =
    { $n ->
        [one] 1 minute
       *[other] { $n } minutes
    }
duration-second =
    { $n ->
        [one] 1 second
       *[other] { $n } seconds
    }

# Settings window
settings-title = Settings
provider = Provider
api-key = API key
add-provider = Add provider
save = Save
