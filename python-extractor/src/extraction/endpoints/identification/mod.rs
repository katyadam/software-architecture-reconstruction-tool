mod decorator;
mod django;
mod flask;
mod method_decorator;
mod strategy;

pub(crate) use decorator::{DecoratorEndpointMatch, collect_decorator_matches, decorator_query};
pub(crate) use strategy::IdentificationStrategy;

use django::{DjangoApiViewIdentificationStrategy, DjangoUrlpatternsIdentificationStrategy};
use flask::FlaskRouteIdentificationStrategy;
use method_decorator::MethodDecoratorIdentificationStrategy;

static METHOD_DECORATOR_IDENTIFICATION: MethodDecoratorIdentificationStrategy =
    MethodDecoratorIdentificationStrategy;
static FLASK_ROUTE_IDENTIFICATION: FlaskRouteIdentificationStrategy =
    FlaskRouteIdentificationStrategy;
static DJANGO_API_VIEW_IDENTIFICATION: DjangoApiViewIdentificationStrategy =
    DjangoApiViewIdentificationStrategy;
static DJANGO_URLPATTERNS_IDENTIFICATION: DjangoUrlpatternsIdentificationStrategy =
    DjangoUrlpatternsIdentificationStrategy;

pub(super) fn strategies() -> [&'static dyn IdentificationStrategy; 4] {
    [
        &METHOD_DECORATOR_IDENTIFICATION,
        &FLASK_ROUTE_IDENTIFICATION,
        &DJANGO_API_VIEW_IDENTIFICATION,
        &DJANGO_URLPATTERNS_IDENTIFICATION,
    ]
}
