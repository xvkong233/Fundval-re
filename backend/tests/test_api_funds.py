"""
测试基金相关 API

测试点：
1. 基金列表（分页、搜索）
2. 基金详情
3. 获取估值
4. 获取准确率
5. 同步基金列表（管理员）
"""
import pytest
from decimal import Decimal
from datetime import date
from rest_framework.test import APIClient
from django.contrib.auth import get_user_model

User = get_user_model()


@pytest.mark.django_db
class TestFundListAPI:
    """测试基金列表 API"""

    @pytest.fixture
    def client(self):
        return APIClient()

    @pytest.fixture
    def user(self):
        return User.objects.create_user(username='testuser', password='pass')

    @pytest.fixture
    def admin_user(self):
        return User.objects.create_superuser(username='admin', password='pass')

    @pytest.fixture
    def funds(self):
        from api.models import Fund
        return [
            Fund.objects.create(fund_code='000001', fund_name='华夏成长混合', fund_type='混合型'),
            Fund.objects.create(fund_code='000002', fund_name='华夏大盘精选', fund_type='混合型'),
            Fund.objects.create(fund_code='110022', fund_name='易方达消费行业', fund_type='股票型'),
        ]

    def test_list_funds_unauthenticated(self, client, funds):
        """测试未认证用户可以查看基金列表"""
        response = client.get('/api/funds/')
        assert response.status_code == 200
        assert len(response.data['results']) == 3

    def test_list_funds_with_pagination(self, client, funds):
        """测试分页"""
        response = client.get('/api/funds/?page_size=2')
        assert response.status_code == 200
        assert len(response.data['results']) == 2
        assert response.data['count'] == 3

    def test_search_funds_by_code(self, client, funds):
        """测试按代码搜索"""
        response = client.get('/api/funds/?search=000001')
        assert response.status_code == 200
        assert len(response.data['results']) == 1
        assert response.data['results'][0]['fund_code'] == '000001'

    def test_search_funds_by_name(self, client, funds):
        """测试按名称搜索"""
        response = client.get('/api/funds/?search=华夏')
        assert response.status_code == 200
        assert len(response.data['results']) == 2

    def test_filter_funds_by_type(self, client, funds):
        """测试按类型过滤"""
        response = client.get('/api/funds/?fund_type=股票型')
        assert response.status_code == 200
        assert len(response.data['results']) == 1
        assert response.data['results'][0]['fund_code'] == '110022'


@pytest.mark.django_db
class TestFundDetailAPI:
    """测试基金详情 API"""

    @pytest.fixture
    def client(self):
        return APIClient()

    @pytest.fixture
    def fund(self):
        from api.models import Fund
        return Fund.objects.create(
            fund_code='000001',
            fund_name='华夏成长混合',
            fund_type='混合型',
            yesterday_nav=Decimal('1.5000'),
            yesterday_date=date(2024, 2, 10),
        )

    def test_get_fund_detail(self, client, fund):
        """测试获取基金详情"""
        response = client.get(f'/api/funds/{fund.fund_code}/')
        assert response.status_code == 200
        assert response.data['fund_code'] == '000001'
        assert response.data['fund_name'] == '华夏成长混合'
        assert Decimal(response.data['yesterday_nav']) == Decimal('1.5000')

    def test_get_nonexistent_fund(self, client):
        """测试获取不存在的基金"""
        response = client.get('/api/funds/999999/')
        assert response.status_code == 404


@pytest.mark.django_db
class TestFundEstimateAPI:
    """测试基金估值 API"""

    @pytest.fixture
    def client(self):
        return APIClient()

    @pytest.fixture
    def fund(self):
        from api.models import Fund
        return Fund.objects.create(
            fund_code='000001',
            fund_name='华夏成长混合',
        )

    def test_get_fund_estimate(self, client, fund, mocker):
        """测试获取基金估值"""
        # Mock 数据源
        mock_source = mocker.Mock()
        mock_source.fetch_estimate.return_value = {
            'fund_code': '000001',
            'fund_name': '华夏成长混合',
            'estimate_nav': Decimal('1.1370'),
            'estimate_growth': Decimal('-1.05'),
            'estimate_time': '2024-02-11 15:00',
        }

        mocker.patch('api.sources.SourceRegistry.get_source', return_value=mock_source)

        response = client.get(f'/api/funds/{fund.fund_code}/estimate/')
        assert response.status_code == 200
        assert Decimal(response.data['estimate_nav']) == Decimal('1.1370')

    def test_get_fund_estimate_with_source(self, client, fund, mocker):
        """测试指定数据源获取估值"""
        mock_source = mocker.Mock()
        mock_source.fetch_estimate.return_value = {
            'fund_code': '000001',
            'estimate_nav': Decimal('1.1370'),
        }

        mocker.patch('api.sources.SourceRegistry.get_source', return_value=mock_source)

        response = client.get(f'/api/funds/{fund.fund_code}/estimate/?source=eastmoney')
        assert response.status_code == 200


@pytest.mark.django_db
class TestFundAccuracyAPI:
    """测试基金准确率 API"""

    @pytest.fixture
    def client(self):
        return APIClient()

    @pytest.fixture
    def fund(self):
        from api.models import Fund
        return Fund.objects.create(
            fund_code='000001',
            fund_name='华夏成长混合',
        )

    @pytest.fixture
    def accuracy_records(self, fund):
        from api.models import EstimateAccuracy
        records = []
        for i in range(10):
            record = EstimateAccuracy.objects.create(
                source_name='eastmoney',
                fund=fund,
                estimate_date=date(2024, 2, i + 1),
                estimate_nav=Decimal('1.1000'),
                actual_nav=Decimal('1.1100'),
                error_rate=Decimal('0.009009'),
            )
            records.append(record)
        return records

    def test_get_fund_accuracy(self, client, fund, accuracy_records):
        """测试获取基金准确率"""
        response = client.get(f'/api/funds/{fund.fund_code}/accuracy/')
        assert response.status_code == 200
        assert 'eastmoney' in response.data
        assert 'avg_error_rate' in response.data['eastmoney']
        assert 'record_count' in response.data['eastmoney']


@pytest.mark.django_db
class TestSyncFundsAPI:
    """测试同步基金列表 API"""

    @pytest.fixture
    def client(self):
        return APIClient()

    @pytest.fixture
    def admin_user(self):
        return User.objects.create_superuser(username='admin', password='pass')

    @pytest.fixture
    def user(self):
        return User.objects.create_user(username='user', password='pass')

    def test_sync_funds_as_admin(self, client, admin_user, mocker):
        """测试管理员同步基金列表"""
        client.force_authenticate(user=admin_user)

        # Mock 数据源
        mock_source = mocker.Mock()
        mock_source.fetch_fund_list.return_value = [
            {'fund_code': '000001', 'fund_name': '华夏成长混合', 'fund_type': '混合型'},
        ]
        mocker.patch('api.sources.SourceRegistry.get_source', return_value=mock_source)

        response = client.post('/api/funds/sync/')
        assert response.status_code == 200
        assert 'created' in response.data
        assert 'updated' in response.data

    def test_sync_funds_as_regular_user(self, client, user):
        """测试普通用户不能同步基金列表"""
        client.force_authenticate(user=user)

        response = client.post('/api/funds/sync/')
        assert response.status_code == 403

    def test_sync_funds_unauthenticated(self, client):
        """测试未认证用户不能同步基金列表"""
        response = client.post('/api/funds/sync/')
        assert response.status_code == 401
