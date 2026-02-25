from pydantic import BaseModel


class Settings(BaseModel):
    service_name: str = "quant-service"


settings = Settings()

