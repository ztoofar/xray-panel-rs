import { useTranslation } from 'react-i18next';
import { Tabs } from 'antd';
import { ExperimentOutlined, GlobalOutlined } from '@ant-design/icons';

import type { AllSetting } from '@/models/setting';
import { useMediaQuery } from '@/hooks/useMediaQuery';
import { catTabLabel } from './catTabLabel';
import AdvancedTab from './AdvancedTab';
import IpDataCloudTab from './IpDataCloudTab';

interface AdvancedTabsProps {
  allSetting: AllSetting;
  updateSetting: (patch: Partial<AllSetting>) => void;
}

export default function AdvancedTabs({ allSetting, updateSetting }: AdvancedTabsProps) {
  const { t } = useTranslation();
  const { isMobile } = useMediaQuery();

  return (
    <Tabs
      defaultActiveKey="outbound"
      items={[
        {
          key: 'outbound',
          label: catTabLabel(
            <ExperimentOutlined aria-hidden />,
            t('pages.settings.outboundCheckTab'),
            isMobile,
          ),
          children: <AdvancedTab allSetting={allSetting} updateSetting={updateSetting} />,
        },
        {
          key: 'ipdata',
          label: catTabLabel(
            <GlobalOutlined aria-hidden />,
            t('pages.settings.ipDataCloud'),
            isMobile,
          ),
          children: <IpDataCloudTab allSetting={allSetting} updateSetting={updateSetting} />,
        },
      ]}
    />
  );
}
