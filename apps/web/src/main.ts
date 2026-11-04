import { createApp } from 'vue';
import { parseRunnerInfo, VeyraClient } from '@core/client-sdk';
import { installClient } from '@core/vue-bindings';
import { installUiKit } from '@ui/kit';
import './app.css';
import WorkspaceView from './WorkspaceView.vue';

if (!__VEYRA_DEV_KEY__) throw new Error('Нет ключа пользователя: вход появится в следующих этапах');
const app = createApp(WorkspaceView);
installUiKit(app);
const client = new VeyraClient(__VEYRA_RELAY__, __VEYRA_DEV_KEY__, parseRunnerInfo(__VEYRA_RUNNER__));
installClient(app, client);
client.setWorkspace(localStorage.getItem('veyra.workspace'));
client.start();
app.mount('#app');
