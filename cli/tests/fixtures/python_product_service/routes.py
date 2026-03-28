from fastapi import FastAPI

app = FastAPI()


@app.get("/api/v1/products/")
async def list_products():
    return []


@app.get("/api/v1/products/{product_id}")
async def get_product(product_id: int):
    return {}


@app.post("/api/v1/products/")
async def create_product(name: str, price: float):
    return {}


@app.delete("/api/v1/products/{product_id}")
async def delete_product(product_id: int):
    return {}
