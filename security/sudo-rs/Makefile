PORTNAME=	sudo-rs
PORTVERSION=	0.2.5
CATEGORIES=	security

MAINTAINER=	marc@trifectatech.org
COMMENT=	Allow others to run commands as root
WWW=		https://trifectatech.org/initiatives/privilege-boundary/

LICENSE=		MIT APACHE20
LICENSE_COMB=		dual
LICENSE_FILE_MIT=	${WRKSRC}/LICENSE-MIT
LICENSE_FILE_APACHE20=	${WRKSRC}/LICENSE-APACHE

USES=			cargo

USE_GITHUB=		yes
GH_ACCOUNT=		trifectatechfoundation
DISTVERSIONPREFIX=	v
BUILD_DEPENDS=		pandoc:${PORTSDIR}/textproc/hs-pandoc

OPTIONS_DEFINE=		COEXIST DOCS
OPTIONS_DEFAULT=	COEXIST
OPTIONS_SUB=yes

COEXIST_DESC=		Install using -rs prefix to coexist with security/sudo

PORTDOCS=	CONTRIBUTING.md CHANGELOG.md COPYRIGHT LICENSE-* README.md SECURITY.md

CARGO_CRATES=	diff-0.1.13 \
		glob-0.3.2 \
		libc-0.2.170 \
		log-0.4.26 \
		pretty_assertions-1.4.1 \
		yansi-1.0.1

.include <bsd.port.options.mk>

.if ${PORT_OPTIONS:MCOEXIST}
RS_SUFFIX=		-rs
PLIST_SUB+=		DEFAULTSUDO="@comment "
.else
RS_SUFFIX=
PLIST_SUB+=		DEFAULTSUDO=""
CONFLICTS_INSTALL=	sudo
.endif

PLIST_SUB+=	RS_SUFFIX=${RS_SUFFIX}

post-build:
.for man in sudo.8 visudo.8 sudoers.5
	@${MKDIR} ${WRKDIR}/docs/man
	pandoc -s -t man ${WRKSRC}/docs/man/${man}.md -o ${WRKDIR}/docs/man/${man}
.endfor

post-install:
	${MV} ${STAGEDIR}${PREFIX}/bin/visudo ${STAGEDIR}${PREFIX}/sbin/visudo${RS_SUFFIX}
	${INSTALL_MAN} ${WRKDIR}/docs/man/sudo.8 ${STAGEDIR}${PREFIX}/share/man/man8/sudo${RS_SUFFIX}.8
	${INSTALL_MAN} ${WRKDIR}/docs/man/visudo.8 ${STAGEDIR}${PREFIX}/share/man/man8/visudo${RS_SUFFIX}.8
	${INSTALL_MAN} ${WRKDIR}/docs/man/sudoers.5 ${STAGEDIR}${PREFIX}/share/man/man5/sudoers${RS_SUFFIX}.5
	${MKDIR} ${STAGEDIR}${PREFIX}/etc/sudoers.d
	${MKDIR} ${STAGEDIR}${PREFIX}/etc/pam.d
	${MKDIR} ${STAGEDIR}/var/run/sudo
	${MKDIR} ${STAGEDIR}/var/db/sudo
	# we are not going to try to replace 'su', since that is part of FreeBSD itself
	${RM} ${STAGEDIR}${PREFIX}/bin/su

post-install-COEXIST-off:
	${INSTALL_DATA} ${FILESDIR}/pam.conf ${STAGEDIR}${PREFIX}/etc/pam.d/sudo.default
	${INSTALL_DATA} ${FILESDIR}/sudoers ${STAGEDIR}${PREFIX}/etc/sudoers.dist

post-install-COEXIST-on:
	${MV} ${STAGEDIR}${PREFIX}/bin/sudo ${STAGEDIR}${PREFIX}/bin/sudo${RS_SUFFIX}

post-install-DOCS-on:
	${MKDIR} ${STAGEDIR}${DOCSDIR}
	cd ${WRKSRC} && ${INSTALL_DATA} ${PORTDOCS} ${STAGEDIR}${DOCSDIR}

.include <bsd.port.mk>
