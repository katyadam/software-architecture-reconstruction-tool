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
    new_id = len(items_db) + 1
    new_item = Item(id=new_id, **item.dict())
    items_db[new_id] = new_item
    return new_item
