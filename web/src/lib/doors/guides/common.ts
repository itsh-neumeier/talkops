// Guide sections that apply to the door stations of all vendors: SIP, ring
// routing, video, DTMF and the HTTP functions are vendor neutral in TalkOps.
import type { GuideSection } from '../guide.ts';

export const commonSections: GuideSection[] = [
	{
		id: 'ring',
		title: 'doorGuide.common.ring.title',
		steps: [
			'doorGuide.common.ring.group',
			'doorGuide.common.ring.target',
			'doorGuide.common.ring.buttons'
		]
	},
	{
		id: 'video',
		title: 'doorGuide.common.video.title',
		notes: [
			'doorGuide.common.video.local',
			'doorGuide.common.video.limits',
			'doorGuide.common.video.audio'
		]
	},
	{
		id: 'dtmf',
		title: 'doorGuide.common.dtmf.title',
		steps: [
			'doorGuide.common.dtmf.http',
			'doorGuide.common.dtmf.code',
			'doorGuide.common.dtmf.use',
			'doorGuide.common.dtmf.external'
		],
		notes: ['doorGuide.common.dtmf.twice']
	},
	{
		id: 'http',
		title: 'doorGuide.common.http.title',
		table: {
			columns: ['doorGuide.colFunction', 'doorGuide.colRequirement'],
			rows: [
				['doorGuide.common.http.opener', 'doorGuide.common.http.openerReq'],
				['doorGuide.common.http.picture', 'doorGuide.common.http.pictureReq'],
				['doorGuide.common.http.events', 'doorGuide.common.http.eventsReq'],
				['doorGuide.common.http.webhook', 'doorGuide.common.http.webhookReq']
			]
		}
	},
	{
		id: 'guard',
		title: 'doorGuide.common.guard.title',
		steps: [
			'doorGuide.common.guard.keep',
			'doorGuide.common.guard.unban',
			'doorGuide.common.guard.trust',
			'doorGuide.common.guard.remove'
		]
	},
	{
		id: 'trouble',
		title: 'doorGuide.common.trouble.title',
		table: {
			columns: ['doorGuide.colSymptom', 'doorGuide.colCheck'],
			rows: [
				['doorGuide.common.trouble.none', 'doorGuide.common.trouble.noneCheck'],
				['doorGuide.common.trouble.401', 'doorGuide.common.trouble.401Check'],
				['doorGuide.common.trouble.403', 'doorGuide.common.trouble.403Check'],
				['doorGuide.common.trouble.ok', 'doorGuide.common.trouble.okCheck']
			]
		},
		notes: ['doorGuide.common.trouble.log']
	}
];
