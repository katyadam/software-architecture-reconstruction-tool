from pydantic import BaseModel


class CartItem(BaseModel):
    product_id: int
    quantity: int
    unit_price: float
