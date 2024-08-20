# Zenoh Gamepad

[![Rust](https://github.com/dmweis/zenoh-gamepad/workflows/Rust/badge.svg)](https://github.com/dmweis/zenoh-gamepad/actions)

Simple remote control node that publishes gamepad commands over zenoh

## Message schema

```json
{
    "$schema": "http://json-schema.org/draft-07/schema#",
    "title": "InputMessage",
    "type": "object",
    "required": [
        "gamepads",
        "time"
    ],
    "properties": {
        "gamepads": {
            "type": "object",
            "additionalProperties": {
                "$ref": "#/definitions/GamepadMessage"
            }
        },
        "time": {
            "type": "string",
            "format": "date-time"
        }
    },
    "definitions": {
        "GamepadMessage": {
            "type": "object",
            "required": [
                "axis_state",
                "button_down_event_counter",
                "button_pressed",
                "button_up_event_counter",
                "connected",
                "last_event_time",
                "name"
            ],
            "properties": {
                "axis_state": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "number",
                        "format": "float"
                    }
                },
                "button_down_event_counter": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "integer",
                        "format": "uint",
                        "minimum": 0.0
                    }
                },
                "button_pressed": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "boolean"
                    }
                },
                "button_up_event_counter": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "integer",
                        "format": "uint",
                        "minimum": 0.0
                    }
                },
                "connected": {
                    "type": "boolean"
                },
                "last_event_time": {
                    "type": "string",
                    "format": "date-time"
                },
                "name": {
                    "type": "string"
                }
            }
        }
    }
}
```
