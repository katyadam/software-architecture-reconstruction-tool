from pydantic import BaseModel
from python_order_service.models import Order


class Invoice(BaseModel):
    id: str
    order: Order
    tax: float
