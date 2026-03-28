from typing import Optional
from fastapi import FastAPI

app = FastAPI()


# Edge case 1: @app.patch — PATCH endpoint (not in any existing test)
@app.patch("/items/{item_id}")
async def update_item_partial(item_id: int, update: dict):
    return {"item_id": item_id}


# Edge case 2: Multiple decorators — only the @app.* decorator is processed;
# a non-route decorator is present but should not affect extraction
@app.get("/items/")
async def list_items(skip: int = 0, limit: int = 10):
    return []


# Edge case 3: Optional parameter with None default — documents that
# Optional[str] is preserved as the datatype and "None" as initial_value
@app.get("/search/")
async def search_items(q: Optional[str] = None):
    return {"q": q}
