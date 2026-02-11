"""
同步基金列表命令

从天天基金同步基金列表到数据库
"""
import logging
from django.core.management.base import BaseCommand
from api.sources import SourceRegistry
from api.models import Fund

logger = logging.getLogger(__name__)


class Command(BaseCommand):
    help = '从天天基金同步基金列表'

    def handle(self, *args, **options):
        self.stdout.write('开始同步基金列表...')

        source = SourceRegistry.get_source('eastmoney')
        if not source:
            self.stdout.write(self.style.ERROR('数据源 eastmoney 未注册'))
            return

        try:
            funds = source.fetch_fund_list()
            self.stdout.write(f'获取到 {len(funds)} 个基金')

            created_count = 0
            updated_count = 0

            for fund_data in funds:
                fund, created = Fund.objects.update_or_create(
                    fund_code=fund_data['fund_code'],
                    defaults={
                        'fund_name': fund_data['fund_name'],
                        'fund_type': fund_data['fund_type'],
                    }
                )

                if created:
                    created_count += 1
                else:
                    updated_count += 1

            self.stdout.write(self.style.SUCCESS(
                f'同步完成：新增 {created_count} 个，更新 {updated_count} 个'
            ))

        except Exception as e:
            logger.error(f'同步基金列表失败: {e}')
            self.stdout.write(self.style.ERROR(f'同步失败: {e}'))
            raise
