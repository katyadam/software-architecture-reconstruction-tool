BASE_URL = "http://ts-user-service"
ORDER_URL = "http://ts-order-service"


# Edge case 1: PATCH method — present in HTTP_METHODS but not tested anywhere
async def update_user_partial(client, user_id: str, patch_data: dict):
    response = await client.patch(f"{BASE_URL}/api/v1/users/{user_id}", json=patch_data)
    return response


# Edge case 2: Two REST calls in one function to different services —
# both must be extracted as separate RestCall entries
async def sync_user_and_order(client, user_id: str):
    user = await client.get(f"{BASE_URL}/api/v1/users/{user_id}")
    orders = await client.get(f"{ORDER_URL}/api/v1/orders/user/{user_id}")
    return user, orders


# Edge case 3: F-string URI with multiple path parameters —
# both parameters must appear in the resolved target_uri
async def get_order_item(client, order_id: str, item_id: str):
    response = await client.get(
        f"{ORDER_URL}/api/v1/orders/{order_id}/items/{item_id}"
    )
    return response
