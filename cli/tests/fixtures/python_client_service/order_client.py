import httpx


async def get_orders(client):
    response = await client.get("http://ts-order-service/api/v1/orders/")
    return response.json()


async def create_order(client, order_data):
    response = await client.post("http://ts-order-service/api/v1/orders/", json=order_data)
    return response.json()
