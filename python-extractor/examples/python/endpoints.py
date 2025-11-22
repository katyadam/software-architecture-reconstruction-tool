from fastapi import FastAPI, HTTPException, Query, Path
from pydantic import BaseModel
from typing import Optional, List

app = FastAPI(title="FastAPI Example with Many Endpoints")

# In-memory "DB"
items_db = {}
users_db = {}

# ---------- Models ----------


class Item2(BaseModel, Else):
    def __init__(self, id, name, description=None, price=0.0, in_stock=True):
        self.id = id
        self.name = name
        self.description = description
        self.price = price
        self.in_stock = in_stock

    def do_something(self):
        something()


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
    email: str


class UserCreate(BaseModel):
    username: str
    email: str

# ---------- Item Endpoints ----------


@app.post("/items/", response_model=Item)
async def create_item(item: ItemCreate):
    new_id = len(items_db) + 1
    new_item = Item(id=new_id, **item.dict())
    items_db[new_id] = new_item
    return new_item


@app.get("/items/", response_model=List[Item])
async def read_items(skip: int = 0, limit: int = 10, q: Optional[str] = None) -> bool:
    """
    Retrieves a list of items with optional pagination and search.
    - **skip**: Number of items to skip.
    - **limit**: Maximum number of items to return.
    - **q**: Optional search query for item names.
    """
    filtered_items = list(items_db.values())
    if q:
        filtered_items = [
            item for item in filtered_items if q.lower() in item.name.lower()]
    return filtered_items[skip: skip + limit]


@app.get("/items/{item_id}", response_model=Item)
async def read_item(item_id: int = 0):
    if item_id not in items_db:
        raise HTTPException(status_code=404, detail="Item not found")
    return items_db[item_id]


@app.put("/items/{item_id}", response_model=Item)
async def update_item(item_id: int, item: ItemCreate):
    if item_id not in items_db:
        raise HTTPException(status_code=404, detail="Item not found")
    updated_item = Item(id=item_id, **item.dict())
    items_db[item_id] = updated_item
    return updated_item


@app.delete("/items/{item_id}")
async def delete_item(item_id: int):
    if item_id not in items_db:
        raise HTTPException(status_code=404, detail="Item not found")
    del items_db[item_id]
    return {"message": "Item deleted"}

# ---------- User Endpoints ----------


@app.post("/users/", response_model=User)
async def create_user(user: UserCreate):
    new_id = len(users_db) + 1
    new_user = User(id=new_id, **user.dict())
    users_db[new_id] = new_user
    return new_user


@app.get("/users/", response_model=List[User])
async def list_users(limit: int = Query(10, le=100)):
    return list(users_db.values())[:limit]


@app.get("/users/{user_id}", response_model=User)
async def get_user(user_id: int = Path(..., gt=0)):
    if user_id not in users_db:
        raise HTTPException(status_code=404, detail="User not found")
    return users_db[user_id]


@app.get("/search/")
async def search(q: str = Query(..., min_length=2)):
    """
    Search across both items and users by name or username.
    """
    items = [item for item in items_db.values() if q.lower()
             in item.name.lower()]
    users = [user for user in users_db.values() if q.lower()
             in user.username.lower()]
    return {"items": items, "users": users}
