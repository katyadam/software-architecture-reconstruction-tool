from .api.v3.routes import cases

from .routes import configurations, examinations, frontends, marketplace, scopes, slides


def add_routes_v3(app, late_init):
    cases.add_routes(app, late_init)
    slides.add_routes(app, late_init)
    examinations.add_routes(app, late_init)
    marketplace.add_routes(app, late_init)
    configurations.add_routes(app, late_init)


def add_routes_v3_scopes(app, late_init):
    scopes.add_routes(app, late_init)


def add_routes_v3_frontends(app, late_init):
    frontends.add_routes(app, late_init)
