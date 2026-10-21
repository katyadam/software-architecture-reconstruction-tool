package main

import (
	"context"
	"fmt"
	"golang.org/x/tools/go/packages"
	"sort"
	"strings"
)

func loadSource(ctx context.Context, opts Options) (string, []*packages.Package, error) {
	l, err := prepareLoad(ctx, opts)
	if err != nil {
		return opts.SourceDir, nil, err
	}
	defer l.Close()
	pkgs, err := packages.Load(&l.Config, "./...")
	if err == nil {
		err = packageLoadError(ctx, pkgs)
	}
	return l.Root, pkgs, err
}
func packageLoadError(ctx context.Context, pkgs []*packages.Package) error {
	problems := map[string]bool{}
	packages.Visit(pkgs, nil, func(p *packages.Package) {
		for _, e := range p.Errors {
			problems[e.Error()] = true
		}
	})
	messages := []string{}
	for e := range problems {
		messages = append(messages, e)
	}
	sort.Strings(messages)
	if len(messages) > 0 {
		return fmt.Errorf("Go package loading: %s", strings.Join(messages, "; "))
	}
	return ctx.Err()
}
