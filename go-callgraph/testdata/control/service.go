package service

var seed = Initialize()

func Initialize() int  { return 7 }
func Changed() int     { return 1 }
func Transitive() int  { return Changed() }
func Untouched() int   { return 9 }
func NeverCalled() int { return 99 }

type Valuer interface{ Value() int }
type Constant struct{}

func (Constant) Value() int         { return Changed() }
func ThroughInterface(v Valuer) int { return v.Value() }
