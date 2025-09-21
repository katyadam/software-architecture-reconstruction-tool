from typing import Optional
from pydantic import BaseModel


class Email(BaseModel):
    username: str
    domain: str


class Item2(BaseModel, Else):
    def __init__(self, id, name, description=None, price=0.0, in_stock=True):
        self.id = id
        self.name = name
        self.description = description
        self.price = price
        self.in_stock = in_stock


class Item(BaseModel):
    id: int
    name: str
    description: Optional[str] = None
    price: float
    in_stock: bool


class ItemCreate(BaseModel):
    name: str
    description: Optional[str]
    price: float
    in_stock: bool = True


class User(BaseModel):
    id: int
    username: str
    email: Email


class UserCreate(BaseModel):
    username: str
    email: Email
