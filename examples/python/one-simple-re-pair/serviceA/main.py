from fastapi import FastAPI
from pydantic import BaseModel
from typing import Dict

app = FastAPI()
items_db: Dict[int, "Item"] = {}


class ItemCreate(BaseModel):
    title: str
    qty: int


class Item(BaseModel):
    id: int
    title: str
    qty: int


@app.post("/items/", response_model=Item)
async def create_item(item: ItemCreate):
    a: B = B()
    a.do(5)


class A():
    def do(self, x: int):
        return x + 5


class B(A):
    def do(self, y: int):
        return y + 7
