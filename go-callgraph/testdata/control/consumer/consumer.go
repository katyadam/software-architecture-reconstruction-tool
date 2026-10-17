package consumer

import service "example.com/tia"

func Read() int { return service.Changed() }
